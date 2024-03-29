use crate::{
    comm::{infrs, models},
    comp::{dialer, http, socks5},
};
use async_std::{
    net::{TcpListener, TcpStream, UdpSocket},
    task,
};
use async_tungstenite::tungstenite::{Error, Message, Result};
use futures::{join, AsyncReadExt, AsyncWriteExt, SinkExt, StreamExt};
use log::*;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::comm::cons::BUFF_LEN;

pub fn serv(cfgs: models::ClientConfigs) {
    let addr = cfgs.listen.to_string();
    let arc = Arc::new(cfgs);

    task::block_on(async {
        let socket: TcpListener = TcpListener::bind(addr).await.unwrap();
        info!("listening on {}", socket.local_addr().unwrap());

        while let Some(client) = socket.incoming().next().await {
            if let Ok(local) = client {
                let cfg = arc.clone();
                task::spawn(async {
                    if let Err(e) = handle_client(cfg, local).await {
                        error!("{}", e);
                    }
                });
            }
        }
    });
}

async fn handle_socks5_client(
    stream: TcpStream,
    cfg: &models::ClientConfigs,
    methods: usize,
) -> std::io::Result<()> {
    let mut local = stream;
    let (cmd, dest) = socks5::do_socks5_handshake(&mut local, methods).await?;
    let _ = match cmd {
        models::Cmds::Connect => handle_socks5_connect(local, cfg, dest).await,
        models::Cmds::UdpAssoc => handle_udp_assoc(local, cfg, dest).await,
        models::Cmds::Bind => handle_bind(local, cfg, dest).await,
        _ => Ok(()),
    };
    return Ok(());
}

async fn handle_http_client(
    stream: TcpStream,
    cfg: &models::ClientConfigs,
    pkg: Vec<u8>,
) -> Result<()> {
    let mut buff = vec![0u8; BUFF_LEN];
    buff[0] = pkg[0];
    buff[1] = pkg[1];

    let mut local = stream;
    let mut n = local.read(&mut buff[2..]).await?;
    n += 2;

    let header = &buff[0..n];
    let addr = http::parse_header(header)?;

    info!("connect to {addr}");
    // info!("with header:\n{}", String::from_utf8_lossy(header));

    let mut remote = dialer::dial(cfg, models::Cmds::Connect, &addr).await?;
    if header[0] == b'C' {
        let resp = b"HTTP/1.1 200 Connection Established\r\n\r\n";
        local.write(resp).await?;
    } else {
        let msg = Message::binary(header);
        remote.send(msg).await?;
    }
    infrs::pump_tcp_ws(local, remote).await;
    return Ok(());
}

async fn handle_client(cfgs: Arc<models::ClientConfigs>, stream: TcpStream) -> Result<()> {
    let cfg = &*cfgs;
    let mut buff = vec![0u8; 3];
    let mut local = stream;
    local.read_exact(&mut buff[0..2]).await?;

    let first = buff[0];
    if first == 0x05 {
        let methods = buff[1] as usize;
        handle_socks5_client(local, cfg, methods).await?;
        return Ok(());
    }

    let second = buff[1];
    if (first == b'C' && second == b'O') || (first == b'G' && second == b'E') {
        handle_http_client(local, cfg, buff).await?;
        return Ok(());
    }

    return Err(Error::Io(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!("unsupported header: [{first}, {second}]"),
    )));
}

async fn handle_bind(local: TcpStream, cfgs: &models::ClientConfigs, dest: String) -> Result<()> {
    let remote = dialer::dial(&cfgs, models::Cmds::Bind, &dest).await?;
    info!("bind to {} ok", dest);
    infrs::pump_tcp_ws(local, remote).await;
    return Ok(());
}

async fn handle_socks5_connect(
    local: TcpStream,
    cfgs: &models::ClientConfigs,
    dest: String,
) -> Result<()> {
    info!("connect to {}", dest);
    let mut writer = local;
    match dialer::dial(&cfgs, models::Cmds::Connect, &dest).await {
        Ok(remote) => {
            socks5::reply(&mut writer, 0x00).await;
            let _ = infrs::pump_tcp_ws(writer, remote).await;
        }
        Err(e) => {
            socks5::reply(&mut writer, 0x05).await;
            return Err(e);
        }
    }
    return Ok(());
}

async fn handle_udp_assoc(
    local: TcpStream,
    cfgs: &models::ClientConfigs,
    expt: String,
) -> Result<()> {
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

                match dialer::dial(cfgs, models::Cmds::UdpAssoc, &"").await {
                    Ok(ws_stream) => {
                        debug!("pumping...");
                        let _ = infrs::pump_udp_ws(socket, ws_stream, sig_recv).await;
                        return Ok(());
                    }
                    _ => {}
                }

                let _ = join!(handle);
            }
        }
    }
    // general SOCKS server failure
    socks5::reply(&mut writer, 0x01).await;
    let _ = writer.close().await;
    return Ok(());
}
