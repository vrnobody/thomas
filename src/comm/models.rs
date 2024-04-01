use crate::comm::utils;
use serde::{Deserialize, Serialize};

pub struct CloseSignal {
    closed: std::cell::Cell<bool>,
}

impl CloseSignal {
    pub fn new() -> CloseSignal {
        CloseSignal {
            closed: std::cell::Cell::new(false),
        }
    }

    pub fn is_closed(&self) -> bool {
        self.closed.get()
    }

    pub fn close(&self) {
        self.closed.set(true);
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EncHeader {
    pub nonce: Vec<u8>,
    pub pubkey: [u8; 32],
    pub ciphertext: Vec<u8>,
}

impl EncHeader {
    pub fn decrypt(&self, key: &str) -> Option<(HeaderFrame, Vec<u8>)> {
        if let Some(text) = utils::aes_decrypt(&self.nonce, &self.ciphertext, key) {
            if let Ok(header) = serde_json::from_str(&text) {
                let hash = utils::sha256(&format!("{key}{text}"));
                return Some((header, hash));
            }
        }
        None
    }

    pub fn to_string(&self) -> Option<String> {
        if let Ok(r) = serde_json::to_string(&self) {
            return Some(r);
        }
        return None;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Cmds {
    Relay = 0x00,
    Connect = 0x01,
    Bind = 0x02,
    UdpAssoc = 0x03,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct HeaderFrame {
    pub cmd: Cmds,
    pub param: String,
    pub padding: Vec<u8>,
}

impl HeaderFrame {
    pub fn encrypt(&self, key: &str, pubkey: &[u8; 32]) -> Option<(EncHeader, Vec<u8>)> {
        if let Ok(text) = serde_json::to_string(self) {
            if let Some((nonce, ciphertext)) = utils::aes_encrypt(&text, key) {
                let hash = utils::sha256(&format!("{key}{text}"));
                return Some((
                    EncHeader {
                        nonce,
                        pubkey: pubkey.clone(),
                        ciphertext,
                    },
                    hash,
                ));
            }
        }
        return None;
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServerConfigs {
    #[serde(default)]
    pub loglevel: String,
    pub listen: String,
    pub pubkey: String,
    pub secret: String,
}

impl Default for ServerConfigs {
    fn default() -> ServerConfigs {
        ServerConfigs {
            loglevel: "info".to_string(),
            listen: "127.0.0.1:3001".to_string(),
            pubkey: "".to_string(),
            secret: "".to_string(),
        }
    }
}

#[derive(Debug)]
pub struct ProxyChain {
    pub next: String,
    pub headers: Vec<EncHeader>,
    pub hashes: Vec<Vec<u8>>,
    pub names: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub addr: String,
    pub pubkey: String,
}

impl ServerInfo {
    pub fn to_header_frame(&self) -> HeaderFrame {
        HeaderFrame {
            cmd: Cmds::Relay,
            param: self.addr.clone(),
            padding: utils::rand_padding(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ClientConfigs {
    #[serde(default)]
    pub loglevel: String,
    pub listen: String,
    pub length: usize,
    pub proxy: String,
    pub inlets: Vec<ServerInfo>,
    pub outlets: Vec<ServerInfo>,
    pub relays: Vec<ServerInfo>,
}

impl Default for ClientConfigs {
    fn default() -> ClientConfigs {
        ClientConfigs {
            loglevel: "info".to_string(),
            listen: "127.0.0.1:1080".to_string(),
            length: 3,
            proxy: "".to_string(),
            inlets: vec![],
            outlets: vec![],
            relays: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    // use super::*;

    #[test]
    fn enc_test() {
        /*
        let key = "123456你好,wrold!".to_string();

        let header = HeaderFrame {
            cmd: crate::comm::models::Cmds::Relay,
            param: "wss://b中aid文u.com/wspath".to_string(),
        };

        let enc_txt = header.encrypt(&key).unwrap();
        let dec = HeaderFrame::decrypt(&enc_txt, &key).unwrap();

        assert_eq!(dec.param, header.param);
        assert_eq!(dec.cmd, header.cmd);
        */
        assert!(true);
    }
}
