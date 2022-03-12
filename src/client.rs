use log::*;

mod comm;
mod comp;

fn main() {
    let cfg = parse_args_for_client();
    comm::logging::init(&cfg.loglevel);
    comm::utils::register_ctrl_c_handler();

    let ver = crate::comm::cons::VERSION;
    let name = crate::comm::cons::PKG_NAME;

    println!("{} client v{} starts", name, ver);
    comp::socks5::serv(cfg);
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
