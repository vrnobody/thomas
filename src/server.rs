extern crate openssl_probe;
mod comm;
mod comp;
use log::*;

fn main() {
    let ver = "1.0.2".to_string();
    let cfg = parse_args_for_server(&ver);
    openssl_probe::init_ssl_cert_env_vars();
    comm::logging::init(&cfg.loglevel);
    comm::utils::register_ctrl_c_handler();
    println!("Thomas server v{} starts", &ver);
    comp::ws::serv(cfg);
    info!("app exited");
}

fn parse_args_for_server(ver: &String) -> comm::models::ServerConfigs {
    let config = comm::utils::parse_cmd_args("server".to_string(), &ver);
    match serde_json::from_str(config.as_str()) {
        Ok(c) => return c,
        Err(e) => {
            println!("parse config fail");
            println!("{:?}", e);
            std::process::exit(1);
        },
    }
}
