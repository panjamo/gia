use anyhow::{Context, Result};
use chrono::prelude::*;
use std::fs;
use std::path::PathBuf;

use crate::browser_preview::open_markdown_preview;
use crate::cli::{Config, OutputMode};
use crate::clipboard::write_clipboard;
use crate::logging::{log_error, log_info};

fn get_outputs_dir() -> Result<PathBuf> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    Ok(home_dir.join(".gia").join("outputs"))
}

fn generate_filename_from_prompt(prompt: &str) -> String {
    // Get current timestamp
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");

    // Handle empty prompt
    if prompt.trim().is_empty() {
        return format!("gia_output_{timestamp}.md");
    }

    // Extract first few words and sanitize
    let words: Vec<&str> = prompt
        .split_whitespace()
        .take(4) // Take first 4 words
        .collect();

    if words.is_empty() {
        return format!("gia_output_{timestamp}.md");
    }

    // Join words and sanitize
    let mut sanitized = words.join("_").to_lowercase();

    // Replace invalid filesystem characters
    sanitized = sanitized
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_alphanumeric() || c == '_' || c == '-' => c,
            _ => '_',
        })
        .collect();

    // Remove multiple consecutive underscores
    while sanitized.contains("__") {
        sanitized = sanitized.replace("__", "_");
    }

    // Trim underscores from start and end
    sanitized = sanitized.trim_matches('_').to_string();

    // Limit length to 50 characters
    if sanitized.len() > 50 {
        sanitized.truncate(50);
        sanitized = sanitized.trim_matches('_').to_string();
    }

    // Final check - if we ended up with empty string, use fallback
    if sanitized.is_empty() {
        return format!("gia_output_{timestamp}.md");
    }

    format!("{sanitized}_{timestamp}.md")
}

pub fn output_text(text: &str, config: &Config) -> Result<()> {
    match config.output_mode {
        OutputMode::TempFileWithPreview => {
            log_info("Writing response to output file, copying file path to clipboard, and opening browser preview");

            // Get outputs directory and create it if it doesn't exist
            let outputs_dir = get_outputs_dir()?;
            if !outputs_dir.exists() {
                fs::create_dir_all(&outputs_dir).context("Failed to create outputs directory")?;
                log_info(&format!("Created outputs directory: {outputs_dir:?}"));
            }

            // Create output file with prompt-based name
            let filename = generate_filename_from_prompt(&config.prompt);
            let output_path = outputs_dir.join(filename);

            // Write content to output file
            fs::write(&output_path, text)?;

            // Copy file path to clipboard
            let file_path_str = output_path.to_string_lossy();
            write_clipboard(&file_path_str)?;

            // Open browser preview
            if let Err(e) = open_markdown_preview(text, &output_path) {
                log_error(&format!("Failed to open browser preview: {e}"));
            } else {
                log_info("Opened browser preview");
            }

            log_info(&format!("Output file created at: {file_path_str}"));

            Ok(())
        }

        OutputMode::Clipboard => {
            log_info("Writing response to clipboard");
            write_clipboard(text)
        }
        OutputMode::Stdout => {
            log_info("Writing response to stdout");
            let plain_text = markdown_to_text::convert(text);
            print!("{plain_text}");
            Ok(())
        }
    }
}
