use anyhow::{Context, Result};
use std::fs;
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

pub fn read_text_file(file_path: &str) -> Result<String> {
    log_debug(&format!("Reading text file: {}", file_path));

    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {}", file_path))?;

    log_info(&format!(
        "Read {} characters from file: {}",
        content.len(),
        file_path
    ));
    Ok(content)
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

    // Add text file contents if any are provided
    if !config.text_files.is_empty() {
        log_info(&format!(
            "Processing {} text file(s)",
            config.text_files.len()
        ));

        for file_path in &config.text_files {
            match read_text_file(file_path) {
                Ok(file_content) => {
                    if !file_content.trim().is_empty() {
                        if !input_text.is_empty() {
                            input_text.push_str("\n\n");
                        }
                        input_text.push_str(&format!("=== Content from: {} ===\n", file_path));
                        input_text.push_str(&file_content);
                        if !file_content.ends_with('\n') {
                            input_text.push('\n');
                        }
                    }
                }
                Err(e) => {
                    log_debug(&format!("Failed to read file {}: {}", file_path, e));
                    eprintln!("Warning: Failed to read file '{}': {}", file_path, e);
                    // Continue processing other files
                }
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::OutputMode;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn test_read_text_file_success() {
        let temp_file = NamedTempFile::new().unwrap();
        let content = "Hello, world!\nThis is a test file.";
        fs::write(temp_file.path(), content).unwrap();

        let result = read_text_file(temp_file.path().to_str().unwrap()).unwrap();
        assert_eq!(result, content);
    }

    #[test]
    fn test_read_text_file_nonexistent() {
        let result = read_text_file("nonexistent_file.txt");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Failed to read file"));
    }

    #[test]
    fn test_get_input_text_with_files() {
        let temp_file1 = NamedTempFile::new().unwrap();
        let temp_file2 = NamedTempFile::new().unwrap();

        let content1 = "Content from file 1";
        let content2 = "Content from file 2";

        fs::write(temp_file1.path(), content1).unwrap();
        fs::write(temp_file2.path(), content2).unwrap();

        let mut config = Config {
            prompt: "Test prompt".to_string(),
            use_clipboard_input: false,
            image_sources: vec![],
            text_files: vec![
                temp_file1.path().to_str().unwrap().to_string(),
                temp_file2.path().to_str().unwrap().to_string(),
            ],
            output_mode: OutputMode::Stdout,
            resume_conversation: None,
            resume_last: false,
            list_conversations: None,
            show_conversation: None,
            model: "test-model".to_string(),
        };

        let result = get_input_text(&mut config, None).unwrap();

        assert!(result.contains("Test prompt"));
        assert!(result.contains("=== Content from:"));
        assert!(result.contains(content1));
        assert!(result.contains(content2));
    }

    #[test]
    fn test_get_input_text_empty_files_list() {
        let mut config = Config {
            prompt: "Test prompt".to_string(),
            use_clipboard_input: false,
            image_sources: vec![],
            text_files: vec![],
            output_mode: OutputMode::Stdout,
            resume_conversation: None,
            resume_last: false,
            list_conversations: None,
            show_conversation: None,
            model: "test-model".to_string(),
        };

        let result = get_input_text(&mut config, None).unwrap();
        assert_eq!(result, "Test prompt");
    }
}
