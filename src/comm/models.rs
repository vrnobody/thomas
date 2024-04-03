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
    use super::*;

    #[test]
    fn header_tests() {
        let (pk1, pri1) = utils::generate_x25519_keypair();

        let serv = ServerInfo {
            name: "test".to_string(),
            addr: "ws://127.0.0.1:3001".to_string(),
            pubkey: pk1.to_string(),
        };

        let hf = serv.to_header_frame();

        let (pk2, _) = utils::generate_x25519_keypair();
        let secret1 = utils::b64_to_secret(&pri1).unwrap();
        let pubkey1 = utils::b64_to_pubkey(&pk1).unwrap();

        let pubkey2 = utils::b64_to_pubkey(&pk2).unwrap();
        let bytes = secret1.diffie_hellman(&pubkey2).to_bytes();
        let key = base64::encode(&bytes);

        let (ehf, hash) = hf.encrypt(&key, &pubkey1.to_bytes()).unwrap();
        let (hf2, hash2) = ehf.decrypt(&key).unwrap();

        let (ehf2, hash3) = hf2.encrypt(&key, &pubkey1.to_bytes()).unwrap();

        assert!(hash.eq(&hash2));
        assert!(hash.eq(&hash3));
        let se = ehf.to_string().unwrap();
        let se2 = ehf2.to_string().unwrap();
        assert!(!se.eq(&se2));
        assert_eq!(hf.cmd, hf2.cmd);
        assert_eq!(hf.param, hf2.param);
        assert_eq!(hf.padding, hf2.padding);
    }
}
