// MIT https://raw.githubusercontent.com/WANG-lp/socks5-rs/master/src/main.rs

use crate::comm::{infrs, models};
use async_std::{
    future::timeout,
    net::{TcpListener, TcpStream, UdpSocket},
    task,
};
use async_tungstenite::{
    async_std::{connect_async, ConnectStream},
    tungstenite::{Error, Message, Result},
    WebSocketStream,
};
use futures::{SinkExt, StreamExt};

use log::*;
use std::time::Duration;

async fn try_listen_tcp(
    header: &models::HeaderFrame,
) -> std::io::Result<(TcpListener, std::net::SocketAddr)> {
    let listener: TcpListener = TcpListener::bind(&header.param).await?;
    let addr = listener.local_addr().unwrap();
    Ok((listener, addr))
}

async fn listen_tcp(
    local: WebSocketStream<ConnectStream>,
    listener: TcpListener,
    addr: std::net::SocketAddr,
) {
    let mut ws_writer = local;
    let mut r = vec![0x05, 0x00, 0x00];
    let tail = infrs::socket_addr_to_vec(addr);
    r.extend(&tail);
    debug!("Send back addr: {:?}", r);
    let _ = ws_writer.send(Message::Binary(r)).await;
    debug!("send done");
    if let Ok((tcp_stream, addr)) = listener.accept().await {
        // handle connection
        debug!("recv conn from: {}", addr);
        r = vec![0x05, 0x00, 0x00];
        r.extend(infrs::socket_addr_to_vec(addr));
        let _ = ws_writer.send(Message::Binary(r)).await;
        let _ = infrs::pump_tcp_ws(tcp_stream, ws_writer).await;
    } else {
        debug!("accept error");
    }
}

async fn handle_bind_cmd(
    local: WebSocketStream<ConnectStream>,
    header: models::HeaderFrame,
    duration: Duration,
) {
    task::spawn(async move {
        let mut ws_writer = local;
        if let Ok(Ok((listener, addr))) = timeout(duration, try_listen_tcp(&header)).await {
            listen_tcp(ws_writer, listener, addr).await;
        } else {
            let r = vec![0x05, 0x00, 0x01, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00];
            let _ = ws_writer.send(Message::Binary(r)).await;
        }
    });
}

async fn relay_ws_tcp(
    local: WebSocketStream<ConnectStream>,
    header: models::HeaderFrame,
    span: Duration,
) {
    let addr = &header.param;
    if let Ok(result) = timeout(span, TcpStream::connect(addr)).await {
        if let Ok(remote) = result {
            infrs::pump_tcp_ws(remote, local).await;
        } else {
            info!("dial failed: {}", addr);
        }
    }
}

async fn relay_ws_ws(
    local: WebSocketStream<ConnectStream>,
    header: models::HeaderFrame,
    span: Duration,
) {
    if let Ok(result) = timeout(span, connect_async(&header.param)).await {
        if let Ok((remote, _)) = result {
            infrs::pump_ws_ws(local, remote).await;
        } else {
            info!("dial to [{}] failed!", &header.desc);
        }
    }
}

async fn relay_ws_udp(local: WebSocketStream<ConnectStream>) {
    debug!("prepare to relay udp");
    if let Ok(raw_socket) = UdpSocket::bind("0.0.0.0:0").await {
        if let Ok(addr) = raw_socket.local_addr() {
            info!("Create outbound socket: {}", addr);
            infrs::pump_ws_udp(local, raw_socket).await;
        }
    } else {
        info!("Create outbound udp socket fail!");
    }
}

async fn relay(local: WebSocketStream<ConnectStream>, header: models::HeaderFrame) {
    let duration = crate::comm::cons::CONN_TIMEOUT;
    match header.cmd {
        models::Cmds::Bind => {
            info!("bind on {}", header.param);
            handle_bind_cmd(local, header, duration).await;
        }
        models::Cmds::Relay => {
            info!("relay to {}", header.desc);
            relay_ws_ws(local, header, duration).await;
        }
        models::Cmds::Connect => {
            info!("connect {}", header.param);
            relay_ws_tcp(local, header, duration).await;
        }
        models::Cmds::UdpAssoc => {
            info!("relay socket");
            relay_ws_udp(local).await;
        }
    }
}

async fn read_one_message(
    ws_stream: &mut WebSocketStream<ConnectStream>,
    key: &String,
) -> Option<models::HeaderFrame> {
    if let Some(Ok(msg)) = ws_stream.next().await {
        if msg.is_text() {
            if let Ok(cipher) = msg.into_text() {
                return models::HeaderFrame::decrypt(&cipher, key);
            }
        }
    }
    None
}

async fn accept_connection(
    key: String,
    tcp_stream: TcpStream,
) -> Result<(WebSocketStream<ConnectStream>, models::HeaderFrame)> {
    use async_tungstenite::stream::Stream;
    let stream = Stream::Plain(tcp_stream);
    let mut ws_stream = async_tungstenite::accept_async(stream).await?;
    if let Some(header) = read_one_message(&mut ws_stream, &key).await {
        return Ok((ws_stream, header));
    }
    Err(Error::ConnectionClosed)
}

pub fn serv(cfgs: models::ServerConfigs) {
    task::block_on(run(cfgs)).unwrap();
}

async fn run(cfgs: models::ServerConfigs) -> std::io::Result<()> {
    let addr = cfgs.listen.to_string();
    let key = cfgs.key.to_string();

    let try_socket = TcpListener::bind(&addr).await;
    let err = format!("bind to {} failed", addr);
    let listener = try_socket.expect(err.as_str());
    info!("listening on: {}", addr);

    while let Ok((stream, _)) = listener.accept().await {
        let k = key.to_string();
        let span = crate::comm::cons::CONN_TIMEOUT;
        task::spawn(async move {
            let conn = accept_connection(k, stream);
            if let Ok(Ok((ws_stream, header))) = timeout(span, conn).await {
                relay(ws_stream, header).await;
            }
        });
    }

    Ok(())
}
