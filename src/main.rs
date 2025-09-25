mod api_key;
mod app;
mod cli;
mod clipboard;
mod constants;
mod conversation;
mod gemini;
mod input;
mod logging;
mod output;
mod provider;

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
