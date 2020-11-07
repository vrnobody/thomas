extern crate openssl_probe;
use log::*;

mod comm;
mod comp;

fn main() {
    let ver = "1.0.2".to_string();
    let cfg = parse_args_for_client(&ver);
    openssl_probe::init_ssl_cert_env_vars();
    comm::logging::init(&cfg.loglevel);
    comm::utils::register_ctrl_c_handler();
    println!("Thomas client v{} starts", &ver);
    comp::socks5::serv(cfg);
    info!("app exited");
}

fn parse_args_for_client(ver: &String) -> comm::models::ClientConfigs {
    let config = comm::utils::parse_cmd_args("client".to_string(), &ver);
    match serde_json::from_str(config.as_str()) {
        Ok(c) => {
            return c;
        }
        Err(e) => {
            println!("parse config fail");
            println!("{:?}", e);
            std::process::exit(1);
        }
    }
}
