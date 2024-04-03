// MIT https://raw.githubusercontent.com/WANG-lp/socks5-rs/master/src/main.rs

use crate::comm::cons::CONN_TIMEOUT;
use crate::comm::{infrs, models, utils};
use async_std::{
    future::timeout,
    net::{TcpListener, TcpStream, UdpSocket},
    sync::Arc,
    task,
};
use async_tungstenite::accept_async;
use async_tungstenite::{
    async_std::{connect_async, ConnectStream},
    stream::Stream,
    tungstenite::{Error, Message, Result},
    WebSocketStream,
};
use futures::{SinkExt, StreamExt};
use log::*;
use x25519_dalek::{PublicKey, StaticSecret};

async fn listen_tcp(
    header: &models::HeaderFrame,
) -> std::io::Result<(TcpListener, std::net::SocketAddr)> {
    let listener: TcpListener = TcpListener::bind(&header.param).await?;
    let addr = listener.local_addr().unwrap();
    Ok((listener, addr))
}

async fn accept_tcp_bind_conn(
    mut local: WebSocketStream<ConnectStream>,
    listener: TcpListener,
    addr: std::net::SocketAddr,
) {
    let mut r = vec![0x05, 0x00, 0x00];
    let tail = utils::addr_to_vec(addr);
    r.extend(&tail);
    debug!("Send back addr: {:?}", r);
    let _ = local.send(Message::Binary(r)).await;
    debug!("send done");
    if let Ok((tcp_stream, addr)) = listener.accept().await {
        debug!("recv conn from: {}", addr);
        r = vec![0x05, 0x00, 0x00];
        r.extend(utils::addr_to_vec(addr));
        let _ = local.send(Message::Binary(r)).await;
        let _ = infrs::pump_ws_tcp(tcp_stream, local).await;
    } else {
        debug!("accept error");
    }
}

async fn handle_tcp_bind(mut local: WebSocketStream<ConnectStream>, header: models::HeaderFrame) {
    let r = timeout(CONN_TIMEOUT, listen_tcp(&header)).await;
    if let Ok(Ok((listener, addr))) = r {
        accept_tcp_bind_conn(local, listener, addr).await;
    } else {
        let buf = vec![0x05, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
        let _ = local.send(Message::Binary(buf)).await;
    }
}

async fn relay_ws_tcp(local: WebSocketStream<ConnectStream>, header: models::HeaderFrame) {
    let addr = &header.param;
    if let Ok(result) = timeout(CONN_TIMEOUT, TcpStream::connect(addr)).await {
        if let Ok(remote) = result {
            infrs::pump_ws_tcp(remote, local).await;
        } else {
            info!("dial failed: {}", addr);
        }
    }
}

async fn relay_ws_ws(local: WebSocketStream<ConnectStream>, header: models::HeaderFrame) {
    if let Ok(result) = timeout(CONN_TIMEOUT, connect_async(&header.param)).await {
        if let Ok((remote, _)) = result {
            infrs::pump_ws_ws(local, remote).await;
            return;
        }
    }
    info!("dial to [{}] failed!", &header.param);
}

async fn relay_ws_udp(local: WebSocketStream<ConnectStream>) {
    debug!("prepare to relay udp");
    if let Ok(raw_socket) = UdpSocket::bind("0.0.0.0:0").await {
        if let Ok(addr) = raw_socket.local_addr() {
            info!("Create outbound socket: {}", addr);
            infrs::pump_ws_udp_remote_host(local, raw_socket).await;
        }
    } else {
        info!("Create outbound udp socket fail!");
    }
}

async fn handle_cmd(local: WebSocketStream<ConnectStream>, header: models::HeaderFrame) {
    match header.cmd {
        // ws <- tunnel -> tcp
        models::Cmds::Bind => {
            info!("bind on {}", header.param);
            handle_tcp_bind(local, header).await;
        }
        models::Cmds::Relay => {
            info!("relay to {}", header.param);
            relay_ws_ws(local, header).await;
        }
        models::Cmds::Connect => {
            info!("connect to {}", header.param);
            relay_ws_tcp(local, header).await;
        }
        // ws <- tunnel -> udp
        models::Cmds::UdpAssoc => {
            info!("relay socket");
            relay_ws_udp(local).await;
        }
    }
}

async fn read_one_message(
    ws_stream: &mut WebSocketStream<ConnectStream>,
    secret: Arc<StaticSecret>,
) -> Option<(models::HeaderFrame, Vec<u8>)> {
    if let Ok(Some(Ok(msg))) = timeout(CONN_TIMEOUT, ws_stream.next()).await {
        if msg.is_text() {
            if let Ok(text) = msg.into_text() {
                if let Ok(encrypted) = serde_json::from_str::<models::EncHeader>(&text) {
                    // println!("recv encrypted header:\n{:?}", &encrypted);
                    let their_pubkey = PublicKey::from(encrypted.pubkey.clone());
                    let bytes = secret.diffie_hellman(&their_pubkey).to_bytes();
                    let key = base64::encode(&bytes);
                    return encrypted.decrypt(&key);
                }
            }
        }
    }
    None
}

async fn accept_ws_conn(
    secret: Arc<StaticSecret>,
    tcp_stream: TcpStream,
) -> Result<(WebSocketStream<ConnectStream>, models::HeaderFrame)> {
    let stream = Stream::Plain(tcp_stream);
    if let Ok(Ok(mut ws_stream)) = timeout(CONN_TIMEOUT, accept_async(stream)).await {
        if let Some((header, hash)) = read_one_message(&mut ws_stream, secret).await {
            let msg = Message::binary(hash);
            if let Ok(_) = timeout(CONN_TIMEOUT, ws_stream.send(msg)).await {
                return Ok((ws_stream, header));
            }
        } else {
            infrs::close_ws_stream(ws_stream).await;
        }
    }
    info!("parse header failed");
    Err(Error::ConnectionClosed)
}

pub fn serv(cfgs: models::ServerConfigs) {
    let addr = cfgs.listen.to_string();
    let secret = Arc::new(utils::b64_to_secret(&cfgs.secret).unwrap());

    task::block_on(async {
        let socket = TcpListener::bind(&addr).await.unwrap();
        info!("listening on: {}", addr);

        while let Ok((stream, _)) = socket.accept().await {
            let s = secret.clone();
            task::spawn(async move {
                if let Ok(Ok((ws_stream, header))) =
                    timeout(CONN_TIMEOUT, accept_ws_conn(s, stream)).await
                {
                    let _ = handle_cmd(ws_stream, header).await;
                } else {
                    info!("connection closed");
                }
            });
        }
    });
}
