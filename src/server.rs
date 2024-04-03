extern crate openssl_probe;
use log::*;

mod comm;
mod comp;

fn main() {
    let config = comm::utils::parse_cmd_args(true);
    if config == None {
        std::process::exit(0);
    }

    let cfg = parse_args_for_server(&config.unwrap());
    if !comm::utils::is_keypair(&cfg.secret, &cfg.pubkey) {
        println!("invalid keypair!\nplease run \"server --key\" to generate new keypair");
        std::process::exit(2);
    }

    comm::logging::init(&cfg.loglevel);
    openssl_probe::init_ssl_cert_env_vars();
    comm::utils::register_ctrl_c_handler();

    let ver = crate::comm::cons::VERSION;
    let name = crate::comm::cons::PKG_NAME;
    println!("{} server v{} starts", name, ver);
    comp::ws::serv(cfg);
    info!("{} exits", name);
}

fn parse_args_for_server(config: &str) -> comm::models::ServerConfigs {
    match serde_json::from_str(config) {
        Ok(c) => return c,
        Err(e) => {
            println!("parse config fail");
            println!("{:?}", e);
            std::process::exit(1);
        }
    }
}
