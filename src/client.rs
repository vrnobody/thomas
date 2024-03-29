extern crate openssl_probe;
use log::*;

mod comm;
mod comp;

fn main() {
    openssl_probe::init_ssl_cert_env_vars();

    let cfg = parse_args_for_client();
    comm::logging::init(&cfg.loglevel);
    comm::utils::register_ctrl_c_handler();

    let ver = crate::comm::cons::VERSION;
    let name = crate::comm::cons::PKG_NAME;

    println!("{} client v{} starts", name, ver);
    comp::listener::serv(cfg);
    info!("{} exits", name);
}

fn parse_args_for_client() -> comm::models::ClientConfigs {
    let config = comm::utils::parse_cmd_args(false);
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
