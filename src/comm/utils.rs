use aes_gcm::aead::{generic_array::GenericArray, Aead, NewAead};
use aes_gcm::Aes256Gcm;
use rand::Rng; // Or `Aes128Gcm`
use serde::{Deserialize, Serialize};

pub fn register_ctrl_c_handler() {
    ctrlc::set_handler(move || {
        println!("Detect Ctrl+C!");
        std::process::exit(0);
    })
    .expect("error setting Ctrl-C handler");
}

pub fn parse_cmd_args(is_server: bool) -> String {
    use clap::AppSettings;
    use std::io::Read;

    let name = crate::comm::cons::PKG_NAME;
    let ty = if is_server { "server" } else { "client" };
    let title = format!("{} {}", name, ty);
    let matches = clap::App::new(title)
        .setting(AppSettings::ArgRequiredElseHelp)
        .version(crate::comm::cons::VERSION)
        .author(crate::comm::cons::AUTHORS)
        .args_from_usage("-c, --config=[FILE] 'Load config from file'")
        .args_from_usage("-s, --stdin 'Read config from stdin'")
        .get_matches();

    let mut config = String::new();
    if matches.occurrences_of("stdin") > 0 {
        println!("read config from stdin:");
        if let Ok(_) = std::io::stdin().read_to_string(&mut config) {
            return config;
        }
    } else if let Some(p) = matches.value_of("config") {
        println!("load config from file: {}", p);
        if let Ok(config) = std::fs::read_to_string(p) {
            return config;
        }
    }
    config
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct EncText {
    nonce: Vec<u8>,
    cipher: Vec<u8>,
    padding: Vec<u8>,
}

pub fn aes_dec(enc: &EncText, key: &String) -> Option<String> {
    let hash = sha256(key);
    let key = GenericArray::from_slice(&hash);
    let nonce = GenericArray::from_slice(&enc.nonce);
    let cipher = Aes256Gcm::new(key);
    let plaintext = cipher.decrypt(nonce, enc.cipher.as_ref());
    if let Ok(vu8) = plaintext {
        if let Ok(hf) = std::str::from_utf8(&vu8) {
            return Some(hf.to_string());
        }
    }
    None
}

fn insecure_random_bytes(len: usize) -> Vec<u8> {
    use rand::prelude::*;
    let mut data = vec![0u8; len];
    rand::thread_rng().fill_bytes(&mut data);
    data
}

pub fn aes_enc(text: &String, key: &String) -> Option<EncText> {
    let hash = sha256(key);
    let key = GenericArray::from_slice(&hash);
    let cipher = Aes256Gcm::new(key);
    let nonce = insecure_random_bytes(96 / 8);
    let n = GenericArray::from_slice(&nonce); // 96-bits; unique per message

    if let Ok(ciphertext) = cipher.encrypt(n, text.as_bytes()) {
        return Some(EncText {
            nonce: nonce,
            cipher: ciphertext,
            padding: rand_padding(),
        });
    }
    None
}

pub fn sha256(text: &String) -> Vec<u8> {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let bytes = hasher.finalize();
    bytes.to_vec()
}

pub fn rand_vec8(len: usize) -> Vec<u8> {
    (0..len).map(|_| rand::random::<u8>()).collect()
}

pub fn rand_padding() -> Vec<u8> {
    rand_str(128, 1024)
}

pub fn rand_str(min: usize, max: usize) -> Vec<u8> {
    let mut rng = rand::thread_rng();
    let len = rng.gen_range(min, max);
    rand_vec8(len)
}

pub fn create_node_from(si: &super::models::ServerInfo) -> super::models::HeaderFrame {
    super::models::HeaderFrame {
        cmd: super::models::Cmds::Relay,
        desc: si.name.to_string(),
        param: si.addr.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enc_test() {
        let key = "123456中he文llo".to_string();
        let text = "hello中文1234".to_string();
        let enc = aes_enc(&text, &key).unwrap();
        let dec = aes_dec(&enc, &key).unwrap();

        println!("dec: {:?}", dec);
        assert_eq!(text, dec);
    }
}
