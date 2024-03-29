use std::io::{Error, ErrorKind};

use crate::comm::utils;

pub fn parse_header(buff: &[u8]) -> std::io::Result<String> {
    let header = String::from_utf8_lossy(&buff);
    if header.contains("\r\n\r\n") {
        let ps: Vec<&str> = header.split([' ', '\r', '\n']).collect();
        if ps.len() > 1 {
            let url = ps[1];
            match utils::get_addr(url) {
                Ok(addr) => return Ok(addr),
                Err(_) => return Ok(url.to_string()),
            }
        }
    }
    return Err(Error::new(ErrorKind::InvalidData, "parse header failed!"));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_header_test() {
        parse_header_wrapper("get http://bing.com/ \r\n\r\n", "bing.com:80");
        parse_header_wrapper("get https://bing.com/ \r\n\r\n", "bing.com:443");
        parse_header_wrapper("get http://bing.com:123/ \r\n\r\n", "bing.com:123");
        parse_header_wrapper("get https://bing.com:123/ \r\n\r\n", "bing.com:123");
        parse_header_wrapper("get bing.com:123 \r\n\r\n", "bing.com:123");
        parse_header_wrapper("get bing.com \r\n\r\n", "bing.com");
    }

    fn parse_header_wrapper(header: &str, exp: &str) {
        let addr = parse_header(header.as_bytes()).unwrap();
        println!("addr: [{}] exp: [{}]", addr, exp);
        assert_eq!(addr, exp);
    }
}
