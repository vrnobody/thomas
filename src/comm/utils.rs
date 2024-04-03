use aes_gcm::{
    aead::{generic_array::GenericArray, Aead, NewAead},
    Aes256Gcm,
};
use rand::Rng; // Or `Aes128Gcm`
use rand_chacha::rand_core::SeedableRng;
use rand_chacha::ChaCha20Rng;
use std::convert::TryInto;
use std::io::{Error, ErrorKind};
use url::Url;
use x25519_dalek::{PublicKey, StaticSecret};

pub fn generate_secret() -> StaticSecret {
    let mut rng = ChaCha20Rng::from_entropy();
    StaticSecret::random_from_rng(&mut rng)
}

pub fn generate_x25519_keypair() -> (String, String) {
    let secret = generate_secret();
    let pristr = base64::encode(secret.as_bytes());
    let pubkey = PublicKey::from(&secret);
    let pubstr = base64::encode(pubkey.as_bytes());
    (pubstr, pristr)
}

pub fn is_keypair(secret: &str, pubkey: &str) -> bool {
    if let Some(prikey) = b64_to_secret(secret) {
        let key = PublicKey::from(&prikey);
        let pubstr = base64::encode(key.as_bytes());
        return pubstr.eq(pubkey);
    }
    return false;
}

pub fn b64_to_secret(b64: &str) -> Option<StaticSecret> {
    if let Ok(arr) = base64::decode(&b64) {
        let bytes: [u8; 32] = arr.try_into().unwrap();
        let secret = StaticSecret::from(bytes);
        return Some(secret);
    }
    return None;
}

pub fn b64_to_pubkey(b64: &str) -> Option<PublicKey> {
    if let Ok(arr) = base64::decode(&b64) {
        let bytes: [u8; 32] = arr.try_into().unwrap();
        let pubkey = PublicKey::from(bytes);
        return Some(pubkey);
    }
    return None;
}

#[cfg(test)]
mod tests_dh {
    use super::*;

    #[test]
    fn x25519_keypair_tests() {
        let (pubstr, pristr) = generate_x25519_keypair();
        println!("pub: {pubstr}, pri: {pristr}");
        assert!(is_keypair(&pristr, &pubstr));
        assert!(!is_keypair(&pristr, "hellow, world!"));

        if let Some(prikey) = b64_to_secret(&pristr) {
            if let Some(pubkey) = b64_to_pubkey(&pubstr) {
                assert_eq!(prikey.as_bytes().len(), 32);
                assert_eq!(pubkey.as_bytes().len(), 32);
                assert_eq!(pubstr, base64::encode(pubkey.as_bytes()));
                assert_eq!(pristr, base64::encode(prikey.as_bytes()));

                let derived = PublicKey::from(&prikey);
                assert_eq!(pubstr, base64::encode(derived.as_bytes()));
            } else {
                assert!(false);
            }
        } else {
            assert!(false);
        }
    }
}

// start from ATYPE, then ADDRESS and PORT
pub fn addr_to_vec(socket_addr: std::net::SocketAddr) -> Vec<u8> {
    use bytes::BufMut;

    let mut res = Vec::new();
    let ip_bytes = match socket_addr.ip() {
        std::net::IpAddr::V4(ip) => {
            res.push(0x01);
            ip.octets().to_vec()
        }
        std::net::IpAddr::V6(ip) => {
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

pub fn get_addr(link: &str) -> Result<String, Error> {
    if let Ok(url) = Url::parse(link) {
        if let Some(host) = url.host() {
            if let Some(port) = url.port() {
                return Ok(format!("{host}:{port}"));
            }
            match url.scheme() {
                "https" | "wss" => return Ok(format!("{host}:443")),
                "http" | "ws" | _ => return Ok(format!("{host}:80")),
            }
        }
    }
    return Err(Error::new(ErrorKind::InvalidInput, "parse addr failed"));
}

pub fn register_ctrl_c_handler() {
    ctrlc::set_handler(move || {
        println!("Detect Ctrl+C!");
        std::process::exit(0);
    })
    .expect("error setting Ctrl-C handler");
}

pub fn parse_cmd_args(is_server: bool) -> Option<String> {
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
        .args_from_usage("--key 'Generate random keypairs'")
        .get_matches();

    if matches.occurrences_of("key") > 0 {
        let (pubkey, prikey) = generate_x25519_keypair();
        println!("pubkey: {pubkey}");
        println!("secret: {prikey}");
        return None;
    }

    let mut config = String::new();
    if matches.occurrences_of("stdin") > 0 {
        println!("read config from stdin:");
        if let Ok(_) = std::io::stdin().read_to_string(&mut config) {
            return Some(config);
        }
    }
    if let Some(p) = matches.value_of("config") {
        println!("load config from file: {}", p);
        if let Ok(config) = std::fs::read_to_string(p) {
            return Some(config);
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

pub fn aes_decrypt(nonce: &Vec<u8>, ciphertext: &Vec<u8>, key: &str) -> Option<String> {
    let hash = sha256(key);
    let key = GenericArray::from_slice(&hash);
    let cipher = Aes256Gcm::new(key);
    let plaintext = cipher.decrypt(GenericArray::from_slice(nonce), ciphertext.as_ref());
    if let Ok(arr) = plaintext {
        if let Ok(s) = std::str::from_utf8(&arr) {
            return Some(s.to_string());
        }
    }
    None
}

pub fn aes_encrypt(text: &str, key: &str) -> Option<(Vec<u8>, Vec<u8>)> {
    let hash = sha256(key);
    let key = GenericArray::from_slice(&hash);
    let cipher = Aes256Gcm::new(key);
    let nonce = insecure_random_bytes(96 / 8);
    let n = GenericArray::from_slice(&nonce); // 96-bits; unique per message
    if let Ok(r) = cipher.encrypt(n, text.as_bytes()) {
        return Some((nonce, r));
    }
    None
}

pub fn sha256(text: &str) -> Vec<u8> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enc_test() {
        let key = "123456中he文llo".to_string();
        let text = "hello中文1234".to_string();

        let (nonce, ciphertext) = aes_encrypt(&text, &key).unwrap();
        let r = aes_decrypt(&nonce, &ciphertext, &key).unwrap();

        println!("text: {text} decrpted: {r}");
        assert_eq!(text, r);
    }

    #[test]
    fn get_addr_test() {
        get_addr_wraper("https://bing.com/", "bing.com:443");
        get_addr_wraper("https://bing.com:123/", "bing.com:123");
        get_addr_wraper("http://bing.com", "bing.com:80");
        get_addr_wraper("http://bing.com:123", "bing.com:123");

        // panic get_addr_wraper("bing.com:123", "bing.com:123");
        // panic: get_addr_wraper("bing.com/", "bing.com");
        // panic: get_addr_wraper("bing.com", "bing.com");
    }

    fn get_addr_wraper(url: &str, exp: &str) {
        print!("src: [{url}] exp: [{exp}] ");
        let addr = get_addr(url).unwrap();
        println!("addr: [{addr}]");
        assert_eq!(addr, exp);
    }
}
