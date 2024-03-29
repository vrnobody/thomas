use crate::{
    comm::{models, utils},
    comp,
};
use async_tungstenite::{
    async_std::{client_async_tls, connect_async, ConnectStream},
    tungstenite::{Message, Result},
    WebSocketStream,
};
use futures::SinkExt;
use log::*;

pub async fn dial(
    cfg: &models::ClientConfigs,
    cmd: models::Cmds,
    target: &str,
) -> Result<WebSocketStream<ConnectStream>> {
    match dial_core(&cfg, cmd, &target).await {
        Ok(s) => Ok(s),
        Err(e) => {
            info!("failed to connect: {} {}", target, e);
            Err(e)
        }
    }
}

async fn dial_core(
    cfg: &models::ClientConfigs,
    cmd: models::Cmds,
    target: &str,
) -> Result<WebSocketStream<ConnectStream>> {
    let (mut chain, first) = make_chain(&cfg, cmd, target.to_string());
    let conn;
    if cfg.proxy.is_empty() {
        conn = connect_async(first).await;
    } else {
        // info!("host: {}", &first);
        let s5tcp = comp::proxy::InnerProxy::from_proxy_str(&cfg.proxy)
            .unwrap()
            .connect_async(&first)
            .await
            .unwrap()
            .into_inner();
        conn = client_async_tls(&first, s5tcp).await;
    }
    let (mut ws_stream, _) = conn?;
    while let Some(next) = chain.pop() {
        let msg = Message::text(next);
        ws_stream.send(msg).await?;
    }
    Ok(ws_stream)
}

fn append<'a>(
    tail: &models::HeaderFrame,
    nodes: &mut Vec<String>,
    names: &mut Vec<String>,
    prev: Option<&'a models::ServerInfo>,
    servs: &'a Vec<models::ServerInfo>,
    len: usize,
) -> Option<&'a models::ServerInfo> {
    use rand::seq::SliceRandom;
    let mut r = prev;
    for _ in 0..len {
        let cur = servs.choose(&mut rand::thread_rng());
        if let Some(node) = cur {
            let frame;
            if let Some(p) = r {
                frame = utils::create_node_from(p).encrypt(&node.key);
            } else {
                names.insert(0, tail.param.clone());
                frame = tail.encrypt(&node.key);
            }
            if let Some(c) = frame {
                names.insert(0, node.name.to_string());
                nodes.push(c);
            }
            r = cur.clone();
        }
    }
    return r.clone();
}

fn make_chain(
    cfg: &models::ClientConfigs,
    cmd: models::Cmds,
    target: String,
) -> (Vec<String>, String) {
    let tail = models::HeaderFrame {
        cmd,
        desc: "".to_string(),
        param: target,
    };

    let mut r = vec![];
    let mut names = vec![];

    let prev = append(&tail, &mut r, &mut names, None, &cfg.outlets, 1);
    let prev = append(&tail, &mut r, &mut names, prev, &cfg.relays, cfg.length);
    let prev = append(&tail, &mut r, &mut names, prev, &cfg.inlets, 1);

    let first = prev.unwrap();
    if !cfg.proxy.is_empty() {
        names.insert(0, "proxy".to_string());
    }
    info!("chain: [{}]", names.join(", "));
    (r, first.addr.clone())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn append_node_test() {
        let target = "bing.com:443";
        let tail = models::HeaderFrame {
            cmd: models::Cmds::Connect,
            desc: "".to_string(),
            param: target.to_string(),
        };

        let servs = vec![models::ServerInfo {
            name: "hello".to_string(),
            addr: "ws://127.0.0.1:1234".to_string(),
            key: "123456".to_string(),
        }];

        let len = 4;

        let mut r = vec![];
        let mut names = vec![];

        let prev = append(&tail, &mut r, &mut names, None, &servs, len);

        println!("chain: [{}]", names.join(", "));
        assert_eq!(names.len(), len + 1);
        assert!(!prev.is_none());
        assert_eq!(prev.unwrap().key, servs[0].key);
    }
}
