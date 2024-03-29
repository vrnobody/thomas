// MIT https://raw.githubusercontent.com/WANG-lp/socks5-rs/master/src/main.rs

use crate::comm::models;

use async_std::net::{Ipv4Addr, Ipv6Addr, SocketAddrV4, SocketAddrV6, TcpStream};

use bytes::Buf;
use futures::{AsyncReadExt, AsyncWriteExt};

use std::io::{Error, ErrorKind};

pub async fn do_socks5_handshake(
    local: &mut TcpStream,
    methods: usize,
) -> std::io::Result<(models::Cmds, String)> {
    let mut buffer = vec![0u8; 512];
    local.read_exact(&mut buffer[0..methods]).await?;
    let mut has_no_auth = false;
    for i in 0..methods {
        if buffer[i] == 0x00 {
            has_no_auth = true;
            break;
        }
    }

    if !has_no_auth {
        return Err(Error::new(
            ErrorKind::ConnectionAborted,
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
        return Err(Error::new(
            ErrorKind::AddrNotAvailable,
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
            return Err(Error::new(
                ErrorKind::ConnectionAborted,
                "command is not supported!",
            ));
        }
    }
}

pub async fn reply(local: &mut TcpStream, code: u8) {
    let _ = local
        .write(&[0x05u8, code, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
        .await;
}
