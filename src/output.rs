use anyhow::Result;

use crate::browser_preview::open_markdown_preview;
use crate::cli::{Config, OutputMode};
use crate::clipboard::write_clipboard;
use crate::logging::{log_info, log_error};

pub fn output_text(text: &str, config: &Config) -> Result<()> {
    match config.output_mode {
        OutputMode::ClipboardWithPreview => {
            log_info("Writing response to clipboard and opening browser preview");
            write_clipboard(text)?;
            
            if let Err(e) = open_markdown_preview(text) {
                log_error(&format!("Failed to open browser preview: {}", e));
            } else {
                log_info("Opened browser preview");
            }
            
            Ok(())
        }
        OutputMode::Clipboard => {
            log_info("Writing response to clipboard");
            write_clipboard(text)
        }
        OutputMode::Stdout => {
            log_info("Writing response to stdout");
            print!("{}", text);
            Ok(())
        }
    }
}
