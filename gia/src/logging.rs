use std::env;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;
use tracing::{debug, error, info, trace, warn};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

static LOG_FILE: Mutex<Option<std::fs::File>> = Mutex::new(None);

/// Initialize logging system
/// - Console logging is ONLY enabled when RUST_LOG is set
/// - File logging is enabled when GIA_LOG_TO_FILE is set (per-conversation setup later)
pub fn init_logging() {
    let rust_log_present = env::var("RUST_LOG").is_ok();

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));

    // Only add console layer if RUST_LOG is set
    if rust_log_present {
        let console_layer = fmt::layer()
            .with_target(true)
            .with_file(true)
            .with_line_number(true)
            .with_writer(std::io::stderr);

        tracing_subscriber::registry()
            .with(env_filter)
            .with(console_layer)
            .init();
    } else {
        // Initialize with just the filter, no output layers
        tracing_subscriber::registry().with(env_filter).init();
    }
}

/// Setup file logging for a specific conversation
/// Call this after the conversation ID is known
pub fn setup_conversation_file_logging(conversation_id: &str) -> anyhow::Result<()> {
    if env::var("GIA_LOG_TO_FILE").is_err() {
        return Ok(());
    }

    let conversations_dir = get_conversations_dir()?;

    // Ensure directory exists
    if !conversations_dir.exists() {
        std::fs::create_dir_all(&conversations_dir)?;
    }

    let log_file_path = conversations_dir.join(format!("{}.log", conversation_id));

    // Open or create the log file
    let file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file_path)?;

    // Store the file handle globally
    *LOG_FILE.lock().unwrap() = Some(file);

    log_info(&format!(
        "File logging enabled: {}",
        log_file_path.display()
    ));

    Ok(())
}

fn get_conversations_dir() -> anyhow::Result<PathBuf> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    Ok(home_dir.join(".gia").join("conversations"))
}

// Helper to write to log file if enabled
fn write_to_file(level: &str, target: &str, msg: &str) {
    if let Ok(mut guard) = LOG_FILE.lock()
        && let Some(ref mut file) = *guard
    {
        let timestamp = chrono::Utc::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let _ = writeln!(file, "[{}] [{}] [{}] {}", timestamp, level, target, msg);
        let _ = file.flush();
    }
}

pub fn log_error(msg: &str) {
    error!("{msg}");
    write_to_file("ERROR", "gia", msg);
}

pub fn log_warn(msg: &str) {
    warn!("{msg}");
    write_to_file("WARN", "gia", msg);
}

pub fn log_info(msg: &str) {
    info!("{msg}");
    write_to_file("INFO", "gia", msg);
}

pub fn log_debug(msg: &str) {
    debug!("{msg}");
    write_to_file("DEBUG", "gia", msg);
}

pub fn log_trace(msg: &str) {
    trace!("{msg}");
    write_to_file("TRACE", "gia", msg);
}
