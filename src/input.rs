use anyhow::{Context, Result};
use std::io::{self, Read};

use crate::cli::{Config, ImageSource};
use crate::clipboard::{has_clipboard_image, read_clipboard};
use crate::image::validate_image_file;
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

pub fn get_input_text(config: &mut Config, prompt_override: Option<&str>) -> Result<String> {
    let mut input_text = String::new();

    // Start with command line prompt (or override)
    let prompt_to_use = prompt_override.unwrap_or(&config.prompt);
    if !prompt_to_use.is_empty() {
        log_info("Using command line prompt");
        input_text.push_str(prompt_to_use);
    }

    // Add clipboard input only if requested with -c flag
    if config.use_clipboard_input {
        log_info("Checking clipboard content");

        // First check if there's an image in clipboard
        match has_clipboard_image() {
            Ok(true) => {
                log_info("Found image in clipboard - adding to image sources");
                config.add_clipboard_image();
            }
            Ok(false) => {
                log_info("No image in clipboard, reading text");
                match read_clipboard() {
                    Ok(clipboard_input) => {
                        if !input_text.is_empty() {
                            input_text.push_str("\n\n");
                        }
                        input_text.push_str(&clipboard_input);
                    }
                    Err(e) => {
                        log_debug(&format!("Failed to read clipboard text: {}", e));
                        // Continue without clipboard input
                    }
                }
            }
            Err(e) => {
                log_debug(&format!("Failed to check clipboard for image: {}", e));
                // Fallback to trying text
                match read_clipboard() {
                    Ok(clipboard_input) => {
                        if !input_text.is_empty() {
                            input_text.push_str("\n\n");
                        }
                        input_text.push_str(&clipboard_input);
                    }
                    Err(e) => {
                        log_debug(&format!("Failed to read clipboard text: {}", e));
                    }
                }
            }
        }
    }

    // Always check stdin if available (regardless of flag)
    if atty::isnt(atty::Stream::Stdin) {
        log_info("Stdin data available - adding to input");
        let stdin_input = read_stdin()?;
        if !stdin_input.trim().is_empty() {
            if !input_text.is_empty() {
                input_text.push_str("\n\n");
            }
            input_text.push_str(&stdin_input);
        }
    } else {
        log_debug("No stdin data available (terminal input)");
    }

    Ok(input_text)
}

pub fn validate_image_sources(config: &Config) -> Result<()> {
    if config.image_sources.is_empty() {
        return Ok(());
    }

    log_info(&format!(
        "Validating {} image source(s)",
        config.image_sources.len()
    ));

    for image_source in &config.image_sources {
        match image_source {
            ImageSource::File(image_path) => {
                validate_image_file(image_path)
                    .with_context(|| format!("Failed to validate image file: {}", image_path))?;
            }
            ImageSource::Clipboard => {
                log_debug("Clipboard image source - validation will occur at request time");
                // Note: We can't validate clipboard images ahead of time since clipboard content
                // might change between validation and actual use
            }
        }
    }

    log_info("All image sources validated successfully");
    Ok(())
}
