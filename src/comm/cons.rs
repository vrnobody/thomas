use std::time::Duration;

pub const CONN_TIMEOUT: Duration = Duration::from_secs(60 * 3);
pub const UDP_TIMEOUT: Duration = Duration::from_secs(60 * 30);
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const AUTHORS: &str = env!("CARGO_PKG_AUTHORS");
pub const PKG_NAME: &str = env!("CARGO_PKG_NAME");
pub const BUFF_LEN: usize = 4 * 1024;
