use crate::comm::cons::{BUFF_LEN, CONN_TIMEOUT, UDP_TIMEOUT};
use async_std::{
    future::timeout,
    net::{Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, TcpStream, UdpSocket},
    sync::Arc,
};
use async_tungstenite::{
    async_std::ConnectStream,
    tungstenite::{Error, Message, Result},
    WebSocketStream,
};
use futures::{
    io::{ReadHalf, WriteHalf},
    join,
    stream::{SplitSink, SplitStream},
    AsyncReadExt, AsyncWrite, AsyncWriteExt, SinkExt, StreamExt,
};
use log::*;
use std::sync::atomic;

async fn send_msg_ws<S>(wsw: &mut SplitSink<S, Message>, msg: Message) -> Result<()>
where
    S: futures::Stream<Item = Result<Message>>
        + futures::Sink<Message, Error = async_tungstenite::tungstenite::Error>
        + Unpin,
{
    let finished = Err(Error::ConnectionClosed);

    match msg {
        Message::Close(cm) => {
            let _ = wsw.send(Message::Close(cm)).await;
            finished
        }
        Message::Binary(buff) => {
            let n = buff.len();
            let r = wsw.send(Message::Binary(buff)).await;
            if n < 1 {
                finished
            } else {
                r
            }
        }
        Message::Text(txt) => {
            let n = txt.len();
            let r = wsw.send(Message::Text(txt)).await;
            if n < 1 {
                finished
            } else {
                r
            }
        }
        Message::Ping(_) | Message::Pong(_) => {
            info!("send ping pong websocket message");
            Ok(())
        }
    }
}

async fn send_msg_tcp(tcpw: &mut WriteHalf<TcpStream>, msg: Message) -> Result<()> {
    let finished = Err(Error::ConnectionClosed);
    let _ = match msg {
        Message::Binary(buff) => {
            let r = tcpw.write_all(&buff).await;
            if buff.len() > 0 {
                if let Ok(_) = r {
                    return Ok(());
                }
            }
        }
        Message::Text(txt) => {
            let r = tcpw.write_all(txt.as_bytes()).await;
            if txt.len() > 0 {
                if let Ok(_) = r {
                    return Ok(());
                }
            }
        }
        Message::Ping(_) | Message::Pong(_) => return Ok(()),
        _ => return finished,
    };
    return finished;
}

async fn close_tcp<S>(r: ReadHalf<S>, w: WriteHalf<S>)
where
    S: AsyncWrite + Unpin,
{
    if let Ok(mut stream) = w.reunite(r) {
        let _ = timeout(CONN_TIMEOUT, stream.close()).await;
    }
}

async fn close_ws<S, M>(r: SplitStream<S>, w: SplitSink<S, M>)
where
    S: futures::Sink<M> + Unpin,
{
    if let Ok(mut stream) = w.reunite(r) {
        let _ = timeout(CONN_TIMEOUT, stream.close()).await;
    }
}

async fn copy_ws_ws<S>(wsr: &mut SplitStream<S>, wsw: &mut SplitSink<S, Message>)
where
    S: futures::Stream<Item = Result<Message>>
        + futures::Sink<Message, Error = async_tungstenite::tungstenite::Error>
        + Unpin,
{
    while let Ok(Some(Ok(msg))) = timeout(CONN_TIMEOUT, wsr.next()).await {
        if let Ok(Ok(_)) = timeout(CONN_TIMEOUT, send_msg_ws(wsw, msg)).await {
            continue;
        }
        break;
    }
}

// both directions
pub async fn pump_ws_ws(ws1: WebSocketStream<ConnectStream>, ws2: WebSocketStream<ConnectStream>) {
    debug!("pump ws <-> ws");
    let (mut w1, mut r1) = ws1.split();
    let (mut w2, mut r2) = ws2.split();

    let _ = join!(copy_ws_ws(&mut r2, &mut w1), copy_ws_ws(&mut r1, &mut w2));

    let _ = join!(close_ws(r1, w1), close_ws(r2, w2));
    debug!("ws <= x => ws");
}

pub async fn send_socks5_udp_pkg_to_remote_host(sender: &UdpSocket, buf: &Vec<u8>) {
    use bytes::Buf;
    use std::net::ToSocketAddrs;

    let mut dest: Option<SocketAddr> = None;
    let mut idx = 0usize;
    //processing receved packet from client
    if buf[0] == 0x00 && buf[1] == 0x00 && buf[2] == 0x00 {
        match buf[3] {
            // ipv4
            0x01 => {
                let mut tmp_array: [u8; 4] = Default::default();
                tmp_array.copy_from_slice(&buf[4..8]);
                let v4addr = Ipv4Addr::from(tmp_array);
                let port: u16 = buf[8..10].as_ref().get_u16();
                let socket = SocketAddrV4::new(v4addr, port);
                dest = Some(socket.into());
                idx = 10;
            }
            // domain
            0x03 => {
                let len = buf[4] as usize;
                let port: u16 = buf[5 + len..5 + 2 + len].as_ref().get_u16();
                if let Ok(addr) = std::str::from_utf8(&buf[5..5 + len]) {
                    let addr_port = format!("{}:{}", addr, port);
                    if let Ok(iter) = addr_port.to_socket_addrs() {
                        let servs: Vec<_> = iter.collect();
                        dest = Some(servs[0]);
                        idx = 5 + 2 + len;
                    }
                }
            }
            // ipv6
            0x04 => {
                let mut tmp_array: [u8; 16] = Default::default();
                tmp_array.copy_from_slice(&buf[4..20]);
                let v6addr = Ipv6Addr::from(tmp_array);
                let port: u16 = buf[20..22].as_ref().get_u16();
                let socket = SocketAddrV6::new(v6addr, port, 0, 0);
                dest = Some(socket.into());
                idx = 22;
            }
            // unknown
            _ => {}
        }
    }

    if let Some(addr) = dest {
        let _ = sender.send_to(&buf[idx..], addr).await;
    } else {
        info!("parse addr fail!");
    }
}

async fn copy_ws_udp_to_remote_host(
    wsr: &mut SplitStream<WebSocketStream<ConnectStream>>,
    udpw: &mut Arc<UdpSocket>,
) {
    while let Ok(result) = timeout(CONN_TIMEOUT, wsr.next()).await {
        if let Some(Ok(msg)) = result {
            match msg {
                Message::Binary(buff) => {
                    if buff.len() > 0 {
                        send_socks5_udp_pkg_to_remote_host(&*udpw, &buff).await;
                        continue;
                    }
                }
                Message::Ping(_) | Message::Pong(_) => {
                    debug!("receive ping pong");
                    continue;
                }
                _ => break,
            }
        }
        break;
    }
}

async fn copy_ws_udp_from_remote_host(
    udpr: &mut Arc<UdpSocket>,
    wsw: &mut SplitSink<WebSocketStream<ConnectStream>, Message>,
) {
    let mut buff = vec![0u8; BUFF_LEN];
    while let Ok(Ok((len, s))) = timeout(UDP_TIMEOUT, udpr.recv_from(&mut buff)).await {
        // debug!("Recv udp from {}: len {}", s, len);
        if len > 0 {
            let addr = super::utils::addr_to_vec(s);
            let mut b = vec![0x0u8, 0, 0];
            b.extend(&addr);
            b.extend(&buff[..len]);
            let msg = Message::binary(b);
            if let Ok(Ok(_)) = timeout(CONN_TIMEOUT, wsw.send(msg)).await {
                continue;
            }
        }
        break;
    }
}

pub async fn pump_ws_udp_remote_host(
    ws_stream: WebSocketStream<ConnectStream>,
    udp_socket: UdpSocket,
) {
    let mut udpw = Arc::new(udp_socket);
    let mut udpr = udpw.clone();
    let (mut wsw, mut wsr) = ws_stream.split();

    join!(
        copy_ws_udp_to_remote_host(&mut wsr, &mut udpw),
        copy_ws_udp_from_remote_host(&mut udpr, &mut wsw),
    );

    close_ws(wsr, wsw).await;
    debug!("local ws <= x => outlet udp");
}

async fn get_client_addr_from_first_udp_pkg(
    udpr: &mut Arc<UdpSocket>,
    wsw: &mut SplitSink<WebSocketStream<ConnectStream>, Message>,
) -> Option<SocketAddr> {
    let mut buff = vec![0u8; BUFF_LEN];
    if let Ok(Ok((n, src_addr))) = timeout(UDP_TIMEOUT, udpr.recv_from(&mut buff)).await {
        if n > 0 {
            let msg = Message::binary(&buff[0..n]);
            if let Ok(_) = timeout(CONN_TIMEOUT, wsw.send(msg)).await {
                return Some(src_addr);
            }
        }
    }
    return None;
}

async fn copy_ws_udp_from_local_client(
    udpr: &mut Arc<UdpSocket>,
    wsw: &mut SplitSink<WebSocketStream<ConnectStream>, Message>,
    sig_close: Arc<atomic::AtomicBool>,
) {
    let mut buff = vec![0u8; BUFF_LEN];
    while let Ok(Ok((len, _))) = timeout(UDP_TIMEOUT, udpr.recv_from(&mut buff)).await {
        if sig_close.load(atomic::Ordering::Relaxed) {
            break;
        }
        if len > 0 {
            let msg = Message::binary(&buff[0..len]);
            if let Ok(Ok(_)) = timeout(CONN_TIMEOUT, wsw.send(msg)).await {
                continue;
            }
        }
        break;
    }
}

async fn copy_ws_udp_to_local_client(
    wsr: &mut SplitStream<WebSocketStream<ConnectStream>>,
    udpw: &mut Arc<UdpSocket>,
    client_addr: SocketAddr,
    sig_close: Arc<atomic::AtomicBool>,
) {
    while let Ok(result) = timeout(CONN_TIMEOUT, wsr.next()).await {
        if sig_close.load(atomic::Ordering::Relaxed) {
            break;
        }

        if let Some(Ok(msg)) = result {
            match msg {
                Message::Binary(buff) => {
                    if buff.len() > 0 {
                        if let Ok(_) = timeout(UDP_TIMEOUT, udpw.send_to(&buff, client_addr)).await
                        {
                            continue;
                        } else {
                            break;
                        }
                    }
                }
                Message::Ping(_) | Message::Pong(_) => {
                    debug!("receive ping pong from local client");
                    continue;
                }
                _ => break,
            }
        }
        break;
    }
}

pub async fn pump_ws_udp_local_client(
    udp_socket: UdpSocket,
    ws_stream: WebSocketStream<ConnectStream>,
    sig_close: Arc<atomic::AtomicBool>,
) {
    let mut udpw = Arc::new(udp_socket);
    let mut udpr = udpw.clone();
    let (mut wsw, mut wsr) = ws_stream.split();

    let r = get_client_addr_from_first_udp_pkg(&mut udpr, &mut wsw).await;
    if let Some(src_addr) = r {
        join!(
            copy_ws_udp_to_local_client(&mut wsr, &mut udpw, src_addr, sig_close.clone()),
            copy_ws_udp_from_local_client(&mut udpr, &mut wsw, sig_close),
        );
    }
    close_ws(wsr, wsw).await;
}

pub async fn pump_ws_tcp(tcp_stream: TcpStream, ws_stream: WebSocketStream<ConnectStream>) {
    debug!("pump ws <-> tcp");

    let (mut tcpr, mut tcpw) = tcp_stream.split();
    let (mut wsw, mut wsr) = ws_stream.split();

    let ws2tcp = async {
        while let Ok(Some(Ok(msg))) = timeout(CONN_TIMEOUT, wsr.next()).await {
            if let Ok(Ok(_)) = timeout(CONN_TIMEOUT, send_msg_tcp(&mut tcpw, msg)).await {
                continue;
            }
            break;
        }
    };

    let tcp2ws = async {
        let mut buff = vec![0u8; BUFF_LEN];
        while let Ok(result) = timeout(CONN_TIMEOUT, tcpr.read(&mut buff)).await {
            if let Ok(len) = result {
                if len > 0 {
                    let msg = Message::binary(&buff[0..len]);
                    if let Ok(Ok(_)) = timeout(CONN_TIMEOUT, wsw.send(msg)).await {
                        continue;
                    }
                }
            }
            break;
        }
    };

    join!(ws2tcp, tcp2ws);
    join!(close_ws(wsr, wsw), close_tcp(tcpr, tcpw));

    debug!("tcp <= x => ws");
}
