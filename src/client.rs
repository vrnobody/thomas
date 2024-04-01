extern crate openssl_probe;
use log::*;

mod comm;
mod comp;

fn main() {
    let config = comm::utils::parse_cmd_args(false);
    if config == None {
        std::process::exit(0);
    }

    let cfg = parse_args_for_client(&config.unwrap());
    comm::logging::init(&cfg.loglevel);
    openssl_probe::init_ssl_cert_env_vars();
    comm::utils::register_ctrl_c_handler();

    let ver = crate::comm::cons::VERSION;
    let name = crate::comm::cons::PKG_NAME;
    println!("{} client v{} starts", name, ver);

    comp::listener::serv(cfg);
    info!("{} exits", name);
}

fn parse_args_for_client(config: &str) -> comm::models::ClientConfigs {
    match serde_json::from_str(config) {
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
