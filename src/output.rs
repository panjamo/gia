use anyhow::{Context, Result};
use chrono::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use textwrap;

use crate::browser_preview::{open_markdown_preview, FooterMetadata};
use crate::cli::{Config, ContentSource, OutputMode};
use crate::clipboard::write_clipboard;
use crate::logging::{log_error, log_info};

fn wrap_text(text: &str, width: usize) -> String {
    // First pass: merge lines ending with '•'
    let mut merged_lines = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;
    
    while i < lines.len() {
        let line = lines[i];
        if line.trim_end().ends_with('•') && i + 1 < lines.len() {
            // Remove the '•' and concatenate with next line
            let trimmed = line.trim_end().trim_end_matches('•');
            merged_lines.push(format!("{} {}", trimmed, lines[i + 1]));
            i += 2; // Skip the next line as we've merged it
        } else {
            merged_lines.push(line.to_string());
            i += 1;
        }
    }
    
    // Second pass: wrap the merged lines
    merged_lines
        .iter()
        .map(|line| {
            // Find the position of the first alphanumeric character
            let first_char_pos = line
                .chars()
                .position(|c| c.is_alphanumeric())
                .unwrap_or(0);
            
            // Create indentation string matching the position
            let indent = " ".repeat(first_char_pos);
            
            // Configure textwrap options with subsequent indent
            let options = textwrap::Options::new(width)
                .subsequent_indent(&indent);
            
            textwrap::fill(line, &options)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

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

fn build_footer_metadata(config: &Config) -> FooterMetadata {
    // Parse provider and model from config.model
    let (provider_name, model_name) = if config.model.contains("::") {
        let parts: Vec<&str> = config.model.splitn(2, "::").collect();
        (parts[0].to_string(), parts[1].to_string())
    } else {
        ("gemini".to_string(), config.model.clone())
    };

    // Extract file information from ordered_content
    let mut image_files = Vec::new();
    let mut text_files = Vec::new();
    let mut has_clipboard = false;
    let mut has_audio = false;
    let mut has_stdin = false;

    for content in &config.ordered_content {
        match content {
            ContentSource::ImageFile(path) => {
                if let Some(filename) = Path::new(path).file_name() {
                    image_files.push(filename.to_string_lossy().to_string());
                }
            }
            ContentSource::TextFile(path, _) => {
                if let Some(filename) = Path::new(path).file_name() {
                    text_files.push(filename.to_string_lossy().to_string());
                }
            }
            ContentSource::ClipboardText(_) | ContentSource::ClipboardImage => {
                has_clipboard = true;
            }
            ContentSource::AudioRecording(_) => {
                has_audio = true;
            }
            ContentSource::StdinText(_) => {
                has_stdin = true;
            }
            _ => {}
        }
    }

    FooterMetadata {
        model_name,
        provider_name,
        timestamp: Utc::now(),
        image_files,
        text_files,
        has_clipboard,
        has_audio,
        has_stdin,
        prompt: config.prompt.clone(),
    }
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

            // Build footer metadata
            let metadata = build_footer_metadata(config);

            // Open browser preview with metadata
            if let Err(e) = open_markdown_preview(text, &output_path, Some(&metadata)) {
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
            let plain_text = plain_text.replace('\t', "  ");
            let wrapped_text = wrap_text(&plain_text, 100);
            print!("{wrapped_text}");
            Ok(())
        }
    }
}
