mod api_key;
mod app;
mod audio;
mod browser_preview;
mod cli;
mod clipboard;
mod constants;
mod content_part_wrapper;
mod conversation;
mod gemini;
mod image;
mod input;
mod logging;
mod ollama;
mod output;
mod provider;
mod role;
mod spinner;

use anyhow::Result;

use crate::app::run_app;
use crate::cli::Config;
use crate::logging::init_logging;

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    let config = Config::from_args();
    run_app(config).await
}
