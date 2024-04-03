use crate::{
    comm::{cons::CONN_TIMEOUT, infrs, models, utils},
    comp,
};
use async_std::{future::timeout, stream::StreamExt};
use async_tungstenite::{
    async_std::{client_async_tls, connect_async, ConnectStream},
    tungstenite::{Error, Message, Result},
    WebSocketStream,
};
use futures::SinkExt;
use log::*;
use rand::prelude::SliceRandom;
use x25519_dalek::PublicKey;

pub async fn dial(
    cfg: &models::ClientConfigs,
    cmd: models::Cmds,
    target: &str,
) -> Result<WebSocketStream<ConnectStream>> {
    let tail = models::HeaderFrame {
        cmd,
        param: target.to_string(),
        padding: utils::rand_padding(),
    };
    match dial_core(&cfg, tail).await {
        Ok(s) => Ok(s),
        Err(e) => {
            info!("failed to connect {}", target);
            Err(e)
        }
    }
}

async fn dial_core(
    cfg: &models::ClientConfigs,
    tail: models::HeaderFrame,
) -> Result<WebSocketStream<ConnectStream>> {
    let make_chain_result = make_chain(&cfg, tail);
    if make_chain_result.is_none() {
        warn!("can not create proxy chain");
        return Err(Error::ConnectionClosed);
    }

    let chain = make_chain_result.unwrap();
    info!("chain: [{}]", chain.names.join(", "));

    let conn;
    if cfg.proxy.is_empty() {
        conn = timeout(CONN_TIMEOUT, connect_async(&chain.next)).await;
    } else {
        // info!("host: {}", &first);
        let s5tcp = comp::proxy::InnerProxy::from_proxy_str(&cfg.proxy)
            .unwrap()
            .connect_async(&chain.next)
            .await
            .unwrap()
            .into_inner();
        conn = timeout(CONN_TIMEOUT, client_async_tls(&chain.next, s5tcp)).await;
    }
    if let Ok(Ok((mut ws_stream, _))) = conn {
        for i in 0..chain.headers.len() {
            if let Some(header) = chain.headers[i].to_string() {
                if let Ok(_) = timeout(CONN_TIMEOUT, ws_stream.send(Message::text(header))).await {
                    if let Ok(Some(Ok(msg))) = timeout(CONN_TIMEOUT, ws_stream.next()).await {
                        let hash = msg.into_data();
                        if chain.hashes[i].eq(&hash) {
                            continue;
                        } else {
                            warn!("proxy [{}] reply with incorrect hash", chain.names[i]);
                        }
                    } else {
                        warn!("read hash error");
                    }
                } else {
                    warn!("send header error");
                }
            } else {
                warn!("failed to serialize header");
            }
            infrs::close_ws_stream(ws_stream).await;
            return Err(Error::ConnectionClosed);
        }
        return Ok(ws_stream);
    }
    warn!("fail to connect proxy server");
    return Err(Error::ConnectionClosed);
}

fn make_chain(
    cfg: &models::ClientConfigs,
    tail: models::HeaderFrame,
) -> Option<models::ProxyChain> {
    let secret = utils::generate_secret();
    let pubkey = PublicKey::from(&secret).to_bytes();

    let mut nodes = vec![];
    let mut rng = rand::thread_rng();
    if let Some(node) = cfg.outlets.choose(&mut rng) {
        nodes.push(node);
    }
    for _ in 0..cfg.length {
        if let Some(node) = cfg.relays.choose(&mut rng) {
            nodes.push(node);
        }
    }
    if let Some(node) = cfg.inlets.choose(&mut rng) {
        nodes.push(node);
    }

    if nodes.len() < 1 {
        return None;
    }

    let mut headers = vec![];
    let mut hashes = vec![];
    let mut names = vec![];

    let mut prev: Option<&models::ServerInfo> = None;
    let mut frame = tail;
    let mut name = frame.param.to_string();
    for node in nodes {
        if let Some(their_pubkey) = utils::b64_to_pubkey(&node.pubkey) {
            if let Some(p) = prev {
                name = p.name.to_string();
                frame = p.to_header_frame();
            }
            prev = Some(node);

            let bytes = secret.diffie_hellman(&their_pubkey).to_bytes();
            let key = base64::encode(&bytes);
            if let Some((enc_header, hash)) = frame.encrypt(&key, &pubkey) {
                headers.insert(0, enc_header);
                hashes.insert(0, hash);
                names.insert(0, name.to_string());
            } else {
                return None;
            }
        }
    }

    if let Some(p) = prev {
        names.insert(0, p.name.to_string());
        if !cfg.proxy.is_empty() {
            names.insert(0, "proxy".to_string());
        }
        return Some(models::ProxyChain {
            next: p.addr.to_string(),
            headers,
            hashes,
            names,
        });
    }
    return None;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn make_chain_test() {
        let mut cfg = models::ClientConfigs::default();

        let target = "bing.com:443";
        let tail = models::HeaderFrame {
            cmd: models::Cmds::Connect,
            param: target.to_string(),
            padding: utils::rand_padding(),
        };

        let (serv_pub, _) = utils::generate_x25519_keypair();
        cfg.relays = vec![models::ServerInfo {
            name: "hello".to_string(),
            addr: "ws://127.0.0.1:1234".to_string(),
            pubkey: serv_pub.to_string(),
        }];

        if let Some(chain) = make_chain(&cfg, tail) {
            println!("names: [{}]", chain.names.join(", "));
            println!("hashes len: {}", chain.hashes.len());
            println!("headers len: {}", chain.headers.len());

            assert_eq!(chain.headers.len(), cfg.length);
        } else {
            assert!(false);
        }
    }
}
