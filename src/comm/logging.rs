pub fn init(loglevel: &String) {
    use chrono::Local;
    use std::io::Write;

    let lv = if loglevel.is_empty() {
        "info".to_string()
    } else {
        loglevel.to_string()
    };
    println!("loglevel: {}", lv);
    let env = env_logger::Env::default().filter_or(env_logger::DEFAULT_FILTER_ENV, lv);

    env_logger::Builder::from_env(env)
        .format(|buf, record| {
            writeln!(
                buf,
                "{} {} [{}] {}",
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.module_path().unwrap_or("<unnamed>"),
                &record.args()
            )
        })
        .init();
}
