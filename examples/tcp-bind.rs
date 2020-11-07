use std::io::prelude::*;
use std::net::TcpStream;
use std::thread;

fn main() {
    let addr = "127.0.0.1:1080";
    match tcp_test(addr.to_string()) {
        Ok(_) => println!("Success"),
        _ => println!("Failed!"),
    }
}

fn err(reason: String) -> std::io::Result<()> {
    Err(std::io::Error::new(
        std::io::ErrorKind::ConnectionAborted,
        reason.as_str(),
    ))
}

fn tcp_test(addr: String) -> std::io::Result<()> {
    let mut stream = TcpStream::connect(addr.as_str())?;
    println!("Send client hello");
    stream.write(&vec![0x05, 0x01, 0x00])?;
    let mut b2 = vec![0x0u8; 2];
    stream.read_exact(&mut b2)?;
    if b2[0] == 0x05 && b2[1] == 0x00 {
        println!("Server say hello");
        println!("Bind to 0.0.0.0:0");
        stream.write(&vec![
            0x05, 0x02, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ])?;
        let mut b10 = vec![0x0u8; 10];
        stream.read_exact(&mut b10)?;
        println!("Get reply: {:?}", b10);
        let port = (b10[8] as u16) * 256u16 + (b10[9] as u16);
        // let bind_addr = format!("{}.{}.{}.{}:{}", b10[4], b10[5], b10[6], b10[7], port);
        let bind_addr = format!("127.0.0.1:{}", port);
        println!("Translated addr: {}", bind_addr);
        let handle = thread::spawn(move || -> std::io::Result<()> {
            println!("connecting to remote");
            let mut client = TcpStream::connect(bind_addr)?;
            println!("Connected to bind addr");
            client.write(&vec![0x00, 0x01, 0x02, 0x03, 0x04])?;
            let mut buff = vec![0u8; 1024];
            let mut len = client.read(&mut buff)?;
            println!("Remote recv [{}]: {:?}", len, &buff[..len]);
            client.write(&vec![0x09, 0x08, 0x07, 0x06, 0x05])?;
            len = client.read(&mut buff)?;
            println!("Remote recv [{}]: {:?}", len, &buff[..len]);
            client.shutdown(std::net::Shutdown::Both)?;
            Ok(())
        });

        let mut buff = vec![0u8; 1024];
        // first addr pkg
        let mut len = stream.read(&mut buff)?;
        println!("local recv [{}]: {:?}", len, &buff[..len]);

        // data: 0 1 2 3 4
        len = stream.read(&mut buff)?;
        println!("local recv [{}]: {:?}", len, &buff[..len]);

        // data: 9 8 7 6 5
        stream.write(&buff[..len])?;
        len = stream.read(&mut buff)?;
        println!("local recv [{}]: {:?}", len, &buff[..len]);
        stream.write(&buff[..len])?;
        let _ = handle.join();
    } else {
        return err("Server close connection".to_string());
    }
    stream.shutdown(std::net::Shutdown::Both)?;
    Ok(())
}
