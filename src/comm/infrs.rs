use async_std::{
    future::timeout,
    io,
    net::{
        IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, SocketAddrV4, SocketAddrV6, TcpStream, UdpSocket,
    },
    task,
};
use async_tungstenite::{
    async_std::ConnectStream,
    tungstenite::{Error, Message, Result},
    WebSocketStream,
};
use futures::{join, AsyncReadExt, AsyncWriteExt, SinkExt, StreamExt};
use log::*;
use std::time::Duration;

// start from ATYPE, then ADDRESS and PORT
pub fn socket_addr_to_vec(socket_addr: std::net::SocketAddr) -> Vec<u8> {
    use bytes::BufMut;

    let mut res = Vec::new();
    let ip_bytes = match socket_addr.ip() {
        IpAddr::V4(ip) => {
            res.push(0x01);
            ip.octets().to_vec()
        }
        IpAddr::V6(ip) => {
            res.push(0x04);
            ip.octets().to_vec()
        }
    };
    for val in ip_bytes.iter() {
        res.push(*val);
    }
    res.put_u16(socket_addr.port());
    res
}

async fn send_msg_to_ws_sink(
    msg: Message,
    dest: &mut futures::stream::SplitSink<WebSocketStream<ConnectStream>, Message>,
) -> Result<()> {
    let finished = Err(Error::ConnectionClosed);

    match msg {
        Message::Close(_) => finished,
        Message::Binary(buff) => {
            if buff.len() < 1 {
                finished
            } else {
                dest.send(Message::Binary(buff)).await
            }
        },
        Message::Text(txt) => {
            if txt.len() < 1 {
                finished
            } else {
                dest.send(Message::Text(txt)).await
            }
        },
        Message::Ping(_) => Ok(()),
        _ => finished,
    }
}

async fn send_msg_to_tcp_write_half(
    msg: Message,
    dest: &mut futures::io::WriteHalf<TcpStream>,
) -> io::Result<()> {
    let finished = Err(std::io::Error::new(
        std::io::ErrorKind::WriteZero,
        "Transport completed!",
    ));

    match msg {
        Message::Close(_) => finished,
        Message::Binary(buff) => {
            if buff.len() < 1 {
                finished
            } else {
                // debug!("recv: {:?}", &buff);
                dest.write_all(&buff).await
            }
        },
        Message::Text(txt) => {
            if txt.len() < 1 {
                finished
            } else {
                dest.write_all(txt.as_bytes()).await
            }
        },
        _ => finished,
    }
}

// single direction
async fn copy_ws_ws(
    source: &mut futures::stream::SplitStream<WebSocketStream<ConnectStream>>,
    dest: &mut futures::stream::SplitSink<WebSocketStream<ConnectStream>, Message>,
    span: Duration,
) {
    while let Ok(result) = timeout(span, source.next()).await {
        if let Some(Ok(msg)) = result {
            if let Ok(Ok(_)) = timeout(span, send_msg_to_ws_sink(msg, dest)).await {
                continue;
            }
        }
        break;
    }
}

// both directions
pub async fn pump_ws_ws(
    local: WebSocketStream<ConnectStream>,
    remote: WebSocketStream<ConnectStream>,
) {
    debug!("pump ws <-> ws");
    let (mut lw, mut lr) = local.split();
    let (mut rw, mut rr) = remote.split();

    let timeout = crate::comm::cons::CONN_TIMEOUT;
    let handle = task::spawn(async move {
        let _ = copy_ws_ws(&mut rr, &mut lw, timeout).await;
        let _ = lw.close().await;
    });

    let _ = copy_ws_ws(&mut lr, &mut rw, timeout).await;
    let _ = rw.close().await;
    let _ = join!(handle);
    debug!("ws <= x => ws");
}

pub async fn send_udp_pkg(sender: &UdpSocket, buf: &Vec<u8>) {
    use bytes::Buf;
    use std::net::ToSocketAddrs;

    let mut dest: Option<SocketAddr> = None;
    let mut idx = 0usize;
    //processing receved packet from client
    if buf[0] == 0x00 && buf[1] == 0x00 && buf[2] == 0x00 {
        match buf[3] {
            0x01 => {
                let mut tmp_array: [u8; 4] = Default::default();
                tmp_array.copy_from_slice(&buf[4..8]);
                let v4addr = Ipv4Addr::from(tmp_array);
                let port: u16 = buf[8..10].as_ref().get_u16();
                let socket = SocketAddrV4::new(v4addr, port);
                dest = Some(socket.into());
                idx = 10;
            },
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
            },
            0x04 => {
                let mut tmp_array: [u8; 16] = Default::default();
                tmp_array.copy_from_slice(&buf[4..20]);
                let v6addr = Ipv6Addr::from(tmp_array);
                let port: u16 = buf[20..22].as_ref().get_u16();
                let socket = SocketAddrV6::new(v6addr, port, 0, 0);
                dest = Some(socket.into());
                idx = 22;
            },
            _ => {},
        }
    }

    if let Some(addr) = dest {
        let _ = sender.send_to(&buf[idx..], addr).await;
    } else {
        info!("parse addr fail!");
    }
}

pub async fn pump_ws2udp(ws_stream: WebSocketStream<ConnectStream>, udp_socket: UdpSocket) {
    let udp_sender = std::sync::Arc::new(udp_socket);
    let udp_reader = udp_sender.clone();
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();

    let span = crate::comm::cons::UDP_TIMEOUT;

    let handle = task::spawn(async move {
        let sender = udp_sender;
        while let Ok(result) = timeout(span, ws_receiver.next()).await {
            if let Some(Ok(msg)) = result {
                match msg {
                    Message::Close(_) => break,
                    Message::Binary(buff) => {
                        if buff.len() > 0 {
                            send_udp_pkg(&*sender, &buff).await;
                            continue;
                        }
                    },
                    Message::Ping(_) => continue,
                    Message::Pong(_) => continue,
                    Message::Text(_) => break,
                }
            }
            break;
        }
    });

    let mut buff = vec![0u8; 5 * 1024];
    while let Ok(result) = timeout(span, udp_reader.recv_from(&mut buff)).await {
        if let Ok((len, s)) = result {
            // debug!("Recv udp from {}: len {}", s, len);
            if len > 0 {
                let addr = socket_addr_to_vec(s);
                let mut b = vec![0x0u8, 0, 0];
                b.extend(&addr);
                b.extend(&buff[..len]);
                let msg = Message::binary(b);
                if let Ok(Ok(_)) = timeout(span, ws_sender.send(msg)).await {
                    continue;
                }
            }
        }
        break;
    }
    let _ = ws_sender.close().await;
    let _ = join!(handle);
    debug!("local ws <= x => outlet udp");
}

async fn copy_udp2ws(
    msg: Message,
    udp_sender: std::sync::Arc<UdpSocket>,
    udp_reader: std::sync::Arc<UdpSocket>,
    ws_stream: WebSocketStream<ConnectStream>,
    src: std::net::SocketAddr,
    span: std::time::Duration,
    sig_close: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> Result<()> {
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    if let Err(_) = ws_sender.send(msg).await {
        return Ok(());
    }
    let sig = sig_close.clone();
    let handle = task::spawn(async move {
        let sender = udp_sender;
        while let Ok(result) = timeout(span, ws_receiver.next()).await {
            if sig.load(std::sync::atomic::Ordering::Relaxed) {
                break;
            }
            if let Some(Ok(msg)) = result {
                match msg {
                    Message::Binary(buff) => {
                        if buff.len() > 0 {
                            let _ = sender.send_to(&buff, src).await;
                            continue;
                        }
                    },
                    Message::Ping(_) => continue,
                    Message::Pong(_) => continue,
                    Message::Text(_) => break,
                    Message::Close(_) => break,
                }
            }
            break;
        }
        Ok::<(), Error>(())
    });

    let mut buff = vec![0u8; 4 * 1024];
    while let Ok(result) = timeout(span, udp_reader.recv_from(&mut buff)).await {
        if sig_close.load(std::sync::atomic::Ordering::Relaxed) {
            break;
        }
        if let Ok((len, _)) = result {
            if len > 0 {
                let msg = Message::binary(&buff[0..len]);
                if let Ok(Ok(_)) = timeout(span, ws_sender.send(msg)).await {
                    continue;
                }
            }
        }
        break;
    }
    let _ = ws_sender.close().await;
    let _ = join!(handle);
    debug!("local udp <= x => outlet ws");
    Ok(())
}

pub async fn pump_udp2ws(
    udp_socket: UdpSocket,
    ws_stream: WebSocketStream<ConnectStream>,
    sig_close: std::sync::Arc<std::sync::atomic::AtomicBool>,
) {
    let udp_sender = std::sync::Arc::new(udp_socket);
    let udp_reader = udp_sender.clone();
    let span = crate::comm::cons::UDP_TIMEOUT;
    let mut buff = vec![0u8; 4 * 1024];

    // get client local addr from first udp message
    if let Ok(Ok((len, src))) = timeout(span, udp_reader.recv_from(&mut buff)).await {
        if len > 0 {
            let msg = Message::binary(&buff[..len]);
            let _ = copy_udp2ws(msg, udp_sender, udp_reader, ws_stream, src, span, sig_close).await;
        }
    }
}

pub async fn pump_tcp_ws(
    tcp_stream: TcpStream, 
    ws_stream: WebSocketStream<ConnectStream>
) {
    debug!("pump ws <-> tcp");
    let (mut tcp_reader, mut tcp_writer) = tcp_stream.split();
    let (mut ws_sender, mut ws_receiver) = ws_stream.split();
    let span = crate::comm::cons::CONN_TIMEOUT;

    let handle = task::spawn(async move {
        while let Ok(result) = timeout(span, ws_receiver.next()).await {
            if let Some(Ok(msg)) = result {
                let r = send_msg_to_tcp_write_half(msg, &mut tcp_writer);
                if let Ok(Ok(_)) = timeout(span, r).await
                {
                    continue;
                }
            }
            break;
        }
        let _ = tcp_writer.close().await;
    });

    let mut buff = vec![0u8; 48 * 1024];
    while let Ok(result) = timeout(span, tcp_reader.read(&mut buff)).await {
        if let Ok(len) = result {
            if len > 0 {
                let msg = Message::binary(&buff[0..len]);
                if let Ok(Ok(_)) = timeout(span, ws_sender.send(msg)).await {
                    continue;
                }
            }
        }
        break;
    }

    let _ = ws_sender.close().await;
    let _ = join!(handle);
    debug!("tcp <= x => ws");
}
