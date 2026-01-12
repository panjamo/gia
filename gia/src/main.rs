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
mod tools;

use anyhow::Result;

use crate::app::run_app;
use crate::cli::Config;
use crate::logging::init_logging;

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    // Ensure default task files exist (EN.md and DE.md)
    if let Err(e) = role::ensure_default_tasks() {
        eprintln!("Warning: Failed to create default task files: {}", e);
    }

    let config = Config::from_args();

    // Run app and catch any errors to show in notification
    if let Err(e) = run_app(config.clone()).await {
        // Get the root cause (last in the chain)
        let mut last_cause = e.to_string();
        let mut source = e.source();
        while let Some(cause) = source {
            last_cause = cause.to_string();
            source = cause.source();
        }

        // Extract the most relevant part of the error message
        // Look for "Request failed with status code" pattern
        let error_msg = if let Some(start_idx) = last_cause.find("Request failed with status code")
        {
            if let Some(end_idx) = last_cause[start_idx..].find(". Response body:") {
                format!("Error: {}", &last_cause[start_idx..start_idx + end_idx + 1])
            } else {
                format!("Error: {}", last_cause)
            }
        } else {
            format!("Error: {}", last_cause)
        };

        // Clear clipboard if output mode is clipboard
        if matches!(config.output_mode, crate::cli::OutputMode::Clipboard) {
            use arboard::Clipboard;
            if let Ok(mut clipboard) = Clipboard::new() {
                let _ = clipboard.clear();
            }
        }

        // Show error notification
        #[cfg(target_os = "macos")]
        {
            let _ = std::process::Command::new("osascript")
                .arg("-e")
                .arg(format!(
                    "display notification \"{}\" with title \"GIA Error\"",
                    error_msg.replace('\"', "'")
                ))
                .output();
        }

        #[cfg(not(target_os = "macos"))]
        {
            let _ = Notification::new()
                .summary("GIA Error")
                .body(&error_msg)
                .icon("dialog-error")
                .show();
        }

        // Also return the error for CLI users
        return Err(e);
    }

    Ok(())
}
