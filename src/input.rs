use anyhow::{Context, Result};
use std::io::{self, Read};

use crate::cli::Config;
use crate::clipboard::read_clipboard;
use crate::logging::{log_debug, log_info};

pub fn read_stdin() -> Result<String> {
    log_debug("Reading from stdin");
    let mut buffer = Vec::new();
    io::stdin()
        .read_to_end(&mut buffer)
        .context("Failed to read from stdin")?;

    let text = String::from_utf8_lossy(&buffer).to_string();
    log_info(&format!("Read {} characters from stdin", text.len()));
    Ok(text)
}

pub fn get_input_text(config: &Config, prompt_override: Option<&str>) -> Result<String> {
    let mut input_text = String::new();

    // Start with command line prompt (or override)
    let prompt_to_use = prompt_override.unwrap_or(&config.prompt);
    if !prompt_to_use.is_empty() {
        log_info("Using command line prompt");
        input_text.push_str(prompt_to_use);
    }

    // Add additional input if requested
    if config.use_clipboard_input {
        log_info("Adding clipboard input");
        let clipboard_input = read_clipboard()?;
        if !input_text.is_empty() {
            input_text.push_str("\n\n");
        }
        input_text.push_str(&clipboard_input);
    }

    if config.use_stdin_input {
        log_info("Adding stdin input");
        let stdin_input = read_stdin()?;
        if !input_text.is_empty() {
            input_text.push_str("\n\n");
        }
        input_text.push_str(&stdin_input);
    }

    Ok(input_text)
}
