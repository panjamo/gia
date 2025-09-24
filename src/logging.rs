use log::{error, warn, info, debug, trace};
use std::io::Write;

pub fn init_logging() {
    env_logger::Builder::from_default_env()
        .target(env_logger::Target::Stderr)
        .format(|buf, record| {
            writeln!(
                buf,
                "[{}] [{}:{}] {}",
                record.level(),
                record.file().unwrap_or("unknown"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .init();
}

pub fn log_error(msg: &str) {
    error!("{}", msg);
}

pub fn log_warn(msg: &str) {
    warn!("{}", msg);
}

pub fn log_info(msg: &str) {
    info!("{}", msg);
}

pub fn log_debug(msg: &str) {
    debug!("{}", msg);
}

pub fn log_trace(msg: &str) {
    trace!("{}", msg);
}