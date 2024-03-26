// MIT https://raw.githubusercontent.com/WANG-lp/socks5-rs/master/src/main.rs

use crate::comm::{infrs, models, utils};

use async_std::{
    net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6, TcpListener, TcpStream, UdpSocket},
    task,
};

use async_tungstenite::{
    async_std::{connect_async, ConnectStream},
    tungstenite::{Message, Result},
    WebSocketStream,
};

use bytes::Buf;
use futures::{join, AsyncReadExt, AsyncWriteExt, SinkExt, StreamExt};
use log::*;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

async fn do_socks5_handshake(local: &mut TcpStream) -> std::io::Result<(models::Cmds, String)> {
    // read socks5 header
    let mut buffer = vec![0u8; 512];
    local.read_exact(&mut buffer[0..2]).await?;
    if buffer[0] != 0x05 {
        return Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionAborted,
            "only socks5 protocol is supported!",
        )); // stream will be closed automaticly
    }
    let methods = buffer[1] as usize;
    local.read_exact(&mut buffer[0..methods]).await?;
    let mut has_no_auth = false;
    for i in 0..methods {
        if buffer[i] == 0x00 {
            has_no_auth = true;
        }
    }
    if !has_no_auth {
        return Err(std::io::Error::new(
            std::io::ErrorKind::ConnectionAborted,
            "only no-auth is supported!",
        )); // stream will be closed automaticly
    }

    // server send to client accepted auth method (0x00 no-auth only yet)
    local.write(&[0x05u8, 0x00]).await?;
    local.flush().await?;

    // read socks5 cmd
    local.read_exact(&mut buffer[0..4]).await?;
    let cmd = buffer[1]; // support 0x01(CONNECT) and 0x03(UDP Associate)
    let atype = buffer[3];

    let mut addr_port = String::from("");
    let mut flag_addr_ok = true;

    // parse addr and port first
    match atype {
        0x01 => {
            // ipv4: 4bytes + port
            local.read_exact(&mut buffer[0..6]).await?;
            let mut tmp_array: [u8; 4] = Default::default();
            tmp_array.copy_from_slice(&buffer[0..4]);
            let v4addr = Ipv4Addr::from(tmp_array);
            let port: u16 = buffer[4..6].as_ref().get_u16();
            let socket = SocketAddrV4::new(v4addr, port);
            addr_port = format!("{}", socket);
            // println!("ipv4: {}", addr_port);
        }
        0x03 => {
            local.read_exact(&mut buffer[0..1]).await?;
            let len = buffer[0] as usize;
            local.read_exact(&mut buffer[0..len + 2]).await?;
            let port: u16 = buffer[len..len + 2].as_ref().get_u16();
            if let Ok(addr) = std::str::from_utf8(&buffer[0..len]) {
                addr_port = format!("{}:{}", addr, port);
            } else {
                flag_addr_ok = false;
            }
            // println!("domain: {}", addr_port);
        }
        0x04 => {
            // ipv6: 16bytes + port
            local.read_exact(&mut buffer[0..18]).await?;
            let mut tmp_array: [u8; 16] = Default::default();
            tmp_array.copy_from_slice(&buffer[0..16]);
            let v6addr = Ipv6Addr::from(tmp_array);
            let port: u16 = buffer[16..18].as_ref().get_u16();
            let socket = SocketAddrV6::new(v6addr, port, 0, 0);
            addr_port = format!("{}", socket);
            // println!("ipv6: {}", addr_port);
        }
        _ => {
            flag_addr_ok = false;
        }
    }

    if !flag_addr_ok {
        reply(local, 0x08).await;
        return Err(std::io::Error::new(
            std::io::ErrorKind::AddrNotAvailable,
            "address is not valid!".to_string(),
        ));
    }

    // parse cmd: support CONNECT(0x01) and UDP (0x03) currently
    match cmd {
        0x01 => Ok((models::Cmds::Connect, addr_port)),
        0x02 => Ok((models::Cmds::Bind, addr_port)),
        0x03 => Ok((models::Cmds::UdpAssoc, addr_port)),
        _ => {
            println!("recv udp from: {}", addr_port);
            reply(local, 0x07).await;
            return Err(std::io::Error::new(
                std::io::ErrorKind::ConnectionAborted,
                "command is not supported!",
            ));
        }
    }
}

async fn reply(local: &mut TcpStream, code: u8) {
    let _ = local
        .write(&[0x05u8, code, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
        .await;
}

async fn dial(
    cfg: &models::ClientConfigs,
    cmd: models::Cmds,
    target: &str,
) -> Result<WebSocketStream<ConnectStream>> {
    let (mut chain, first) = make_chain(&cfg, cmd, target.to_string());
    debug!("chain: {} first: {}", chain.len(), first);    
    if let Ok((mut ws_stream, _)) = connect_async(first.as_str()).await {
        while let Some(next) = chain.pop() {
            debug!("send chain objects: {}", next);
            let msg = Message::text(next);
            ws_stream.send(msg).await?;
        }
        Ok(ws_stream)
    } else {
        debug!("dial failed!");
        Err(async_tungstenite::tungstenite::Error::ConnectionClosed)
    }
}

fn append<'a>(
    tail: &models::HeaderFrame,
    nodes: &mut Vec<String>,
    names: &mut Vec<String>,
    prev: Option<&'a models::ServerInfo>,
    servs: &'a Vec<models::ServerInfo>,
    len: usize,
) -> Option<&'a models::ServerInfo> {
    use rand::seq::SliceRandom;
    let mut r = prev;
    for _ in 0..len {
        let cur = servs.choose(&mut rand::thread_rng());
        if let Some(node) = cur {
            let frame;
            if let Some(p) = r {
                frame = utils::create_node_from(p).encrypt(&node.key);
            } else {
                names.insert(0, tail.param.clone());
                frame = tail.encrypt(&node.key);
            }
            if let Some(c) = frame {
                names.insert(0, node.name.to_string());
                nodes.push(c);
            }
            r = cur.clone();
        }
    }
    return r.clone();
}

fn make_chain(
    cfg: &models::ClientConfigs,
    cmd: models::Cmds,
    target: String,
) -> (Vec<String>, String) {
    let tail = models::HeaderFrame {
        cmd,
        desc: "".to_string(),
        param: target,
    };

    let mut r = vec![];
    let mut names = vec![];
    
    let prev = append(&tail, &mut r, &mut names, None, &cfg.outlets, 1);
    let prev = append(&tail, &mut r, &mut names, prev, &cfg.relays, cfg.length);
    let prev = append(&tail, &mut r, &mut names, prev, &cfg.inlets, 1);
    
    let first = prev.unwrap();
    info!("Create chain: [{}] first: {}", names.join(", "), first.name);
    (r, first.addr.clone())
}

async fn handle_bind(cfgs: &models::ClientConfigs, dest: String, local: TcpStream) {
    match dial(&cfgs, models::Cmds::Bind, &dest).await {
        Ok(remote) => {
            info!("bind to {} ok", dest);
            let _ = infrs::pump_tcp_ws(local, remote).await;
        }
        _ => {}
    }
}

async fn handle_connect(cfgs: &models::ClientConfigs, dest: String, local: TcpStream) {
    let mut writer = local.clone();
    match dial(&cfgs, models::Cmds::Connect, &dest).await {
        Ok(remote) => {
            log::info!("connect to {} ok", dest);
            reply(&mut writer, 0x00).await;
            let _ = infrs::pump_tcp_ws(local, remote).await;
        }
        _ => {
            reply(&mut writer, 0x05).await;
        }
    }
}

async fn handle_udp_assoc(cfgs: &models::ClientConfigs, expt: String, local: TcpStream) {
    info!("udp assoc: {}", expt);

    let mut closer = local.clone();
    let mut writer = local;
    if let Ok(socket) = UdpSocket::bind(expt).await {
        if let Ok(addr) = socket.local_addr() {
            let mut resp = vec![0x05u8, 0x00, 0x00];
            let mut bytes = infrs::socket_addr_to_vec(addr);
            resp.append(&mut bytes);
            debug!("bind udp addr: {:?}", resp);
            if let Ok(_) = writer.write(&resp).await {
                let sig_send = Arc::new(AtomicBool::new(false));
                let sig_recv = sig_send.clone();
                let handle = task::spawn(async move {
                    let mut buff = vec![0u8; 1];
                    let _ = closer.read_exact(&mut buff[0..1]).await;
                    sig_send.swap(true, Ordering::Relaxed);
                    let _ = closer.close().await;
                });

                match dial(cfgs, models::Cmds::UdpAssoc, &"").await {
                    Ok(ws_stream) => {
                        debug!("pumping...");
                        let _ = infrs::pump_udp_ws(socket, ws_stream, sig_recv).await;
                        return;
                    }
                    _ => {}
                }

                let _ = join!(handle);
            }
        }
    }
    // general SOCKS server failure
    reply(&mut writer, 0x01).await;
    let _ = writer.close().await;
}

async fn handle_client(cfg: Arc<models::ClientConfigs>, tcp_stream: TcpStream) {
    let mut local = tcp_stream;
    if let Ok((s5cmd, dest)) = do_socks5_handshake(&mut local).await {
        match s5cmd {
            models::Cmds::Connect => handle_connect(&*cfg, dest, local).await,
            models::Cmds::UdpAssoc => handle_udp_assoc(&*cfg, dest, local).await,
            models::Cmds::Bind => handle_bind(&*cfg, dest, local).await,
            _ => {}
        }
    }
}

pub fn serv(cfgs: models::ClientConfigs) {
    let addr = cfgs.listen.to_string();
    let arc = Arc::new(cfgs);

    task::block_on(async {
        let listener: TcpListener = TcpListener::bind(addr).await.unwrap();
        log::info!("listening on {}", listener.local_addr().unwrap());

        while let Some(client) = listener.incoming().next().await {
            if let Ok(local) = client {
                let cfg = arc.clone();
                task::spawn(handle_client(cfg, local));
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_node_test() {
        let target = "bing.com:443";
        let tail = models::HeaderFrame {
            cmd: models::Cmds::Connect,
            desc: "".to_string(),
            param: target.to_string(),
        };

        let servs = vec![models::ServerInfo {
            name: "hello".to_string(),
            addr: "ws://127.0.0.1:1234".to_string(),
            key: "123456".to_string(),
        }];

        let len = 4;

        let mut r = vec![];
        let mut names = vec![];

        let prev = append(&tail, &mut r, &mut names, None, &servs, len);

        println!("chain: [{}]", names.join(", "));
        assert_eq!(names.len(), len + 1);
        assert!(!prev.is_none());
        assert_eq!(prev.unwrap().key, servs[0].key);

    }
}
