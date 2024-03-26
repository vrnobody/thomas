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
    pub ciphertext: Vec<u8>,
    pub padding: Vec<u8>,
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
    pub desc: String,
    pub param: String,
}

impl HeaderFrame {
    pub fn encrypt(&self, key: &String) -> Option<String> {
        if let Ok(text) = serde_json::to_string(self) {
            if let Some(enc) = utils::aes_enc(&text, key) {
                if let Ok(c) = serde_json::to_string(&enc) {
                    return Some(c);
                }
            }
        }
        None
    }

    pub fn decrypt(text: &String, key: &String) -> Option<HeaderFrame> {
        if let Ok(enc_text) = serde_json::from_str(text.as_str()) {
            if let Some(s) = utils::aes_dec(&enc_text, &key) {
                if let Ok(h) = serde_json::from_str(&s) {
                    return Some(h);
                }
            }
        }
        None
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServerConfigs {
    #[serde(default)]
    pub loglevel: String,
    pub listen: String,
    pub key: String,
}

impl Default for ServerConfigs {
    fn default() -> ServerConfigs {
        ServerConfigs {
            loglevel: "info".to_string(),
            listen: "127.0.0.1:3001".to_string(),
            key: "123456".to_string(),
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ServerInfo {
    pub name: String,
    pub addr: String,
    pub key: String,
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
            length: 1,
            proxy: "".to_string(),
            inlets: vec![],
            outlets: vec![],
            relays: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enc_test() {
        let key = "123456你好,wrold!".to_string();

        let header = HeaderFrame {
            cmd: crate::comm::models::Cmds::Relay,
            desc: "se中r文v1".to_string(),
            param: "wss://b中aid文u.com/wspath".to_string(),
        };

        let enc_txt = header.encrypt(&key).unwrap();
        let dec = HeaderFrame::decrypt(&enc_txt, &key).unwrap();

        assert_eq!(dec.desc, header.desc);
        assert_eq!(dec.param, header.param);
        assert_eq!(dec.cmd, header.cmd);
    }
}
