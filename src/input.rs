use anyhow::{Context, Result};
use std::fmt::Write;
use std::fs;
use std::io::{self, Read};

use crate::audio::record_audio;
use crate::cli::{Config, ContentSource, ImageSource};
use crate::clipboard::{has_clipboard_image, read_clipboard};
use crate::image::validate_media_file;
use crate::logging::{log_debug, log_info};
use crate::role::load_all_roles;

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
    log_debug(&format!("Reading text file: {file_path}"));

    let content = fs::read_to_string(file_path)
        .with_context(|| format!("Failed to read file: {file_path}"))?;

    log_info(&format!(
        "Read {} characters from file: {}",
        content.len(),
        file_path
    ));
    Ok(content)
}

pub fn get_input_text(config: &mut Config, prompt_override: Option<&str>) -> Result<String> {
    // Clear any existing ordered content
    config.ordered_content.clear();

    // 0. Role/task definitions (placed first)
    if !config.roles.is_empty() {
        log_info(&format!("Loading {} role(s)/task(s)", config.roles.len()));
        match load_all_roles(&config.roles) {
            Ok(items) => {
                for (name, content, is_task) in items {
                    let item_type = if is_task { "task" } else { "role" };
                    log_info(&format!("Adding {item_type} to ordered content: {name}"));
                    config
                        .ordered_content
                        .push(ContentSource::RoleDefinition(name, content, is_task));
                }
            }
            Err(e) => {
                log_debug(&format!("Failed to load roles/tasks: {e}"));
                eprintln!("Warning: Failed to load roles/tasks: {e}");
            }
        }
    }

    // 1. Command line prompt
    let prompt_to_use = prompt_override.unwrap_or(&config.prompt);
    if !prompt_to_use.is_empty() {
        log_info("Adding command line prompt to ordered content");
        config
            .ordered_content
            .push(ContentSource::CommandLinePrompt(prompt_to_use.to_string()));
    }

    // 2. Audio recording when present
    if config.record_audio {
        log_info("Audio recording requested");
        match record_audio() {
            Ok(audio_path) => {
                log_info(&format!("Audio recorded to: {audio_path}"));
                config
                    .ordered_content
                    .push(ContentSource::AudioRecording(audio_path));

                // If no command line prompt provided, use default audio prompt
                if prompt_to_use.is_empty() {
                    let default_audio_prompt = "Your instructions are in prompt.m4a";
                    log_info(&format!(
                        "Using default audio prompt: {default_audio_prompt}"
                    ));
                    config.ordered_content.insert(
                        0,
                        ContentSource::CommandLinePrompt(default_audio_prompt.to_string()),
                    );
                }
            }
            Err(e) => {
                log_debug(&format!("Audio recording failed: {e}"));
                eprintln!("Warning: Audio recording failed: {e}");
            }
        }
    }

    // 3. Clipboard text when present
    if config.use_clipboard_input {
        log_info("Checking clipboard content");

        match has_clipboard_image() {
            Ok(true) => {
                log_info("Found image in clipboard - adding to ordered content");
                config.ordered_content.push(ContentSource::ClipboardImage);
            }
            Ok(false) => {
                log_info("No image in clipboard, checking for text");
                match read_clipboard() {
                    Ok(clipboard_input) => {
                        if !clipboard_input.trim().is_empty() {
                            log_info("Adding clipboard text to ordered content");
                            config
                                .ordered_content
                                .push(ContentSource::ClipboardText(clipboard_input));
                        }
                    }
                    Err(e) => {
                        log_debug(&format!("Failed to read clipboard text: {e}"));
                    }
                }
            }
            Err(e) => {
                log_debug(&format!("Failed to check clipboard for image: {e}"));
                // Fallback to trying text
                match read_clipboard() {
                    Ok(clipboard_input) => {
                        if !clipboard_input.trim().is_empty() {
                            log_info("Adding clipboard text to ordered content (fallback)");
                            config
                                .ordered_content
                                .push(ContentSource::ClipboardText(clipboard_input));
                        }
                    }
                    Err(_) => {
                        log_debug("Failed to read clipboard text in fallback");
                    }
                }
            }
        }
    }

    // 4. Stdin text if present
    if atty::isnt(atty::Stream::Stdin) {
        log_info("Stdin data available - adding to ordered content");
        let stdin_input = read_stdin()?;
        if !stdin_input.trim().is_empty() {
            config
                .ordered_content
                .push(ContentSource::StdinText(stdin_input));
        }
    } else {
        log_debug("No stdin data available (terminal input)");
    }

    // 5. All files coming with -f option
    if !config.text_files.is_empty() {
        log_info(&format!(
            "Processing {} text file(s)",
            config.text_files.len()
        ));

        for file_path in &config.text_files {
            match read_text_file(file_path) {
                Ok(file_content) => {
                    if !file_content.trim().is_empty() {
                        log_info(&format!("Adding text file to ordered content: {file_path}"));
                        config
                            .ordered_content
                            .push(ContentSource::TextFile(file_path.clone(), file_content));
                    }
                }
                Err(e) => {
                    log_debug(&format!("Failed to read file {file_path}: {e}"));
                    eprintln!("Warning: Failed to read file '{file_path}': {e}");
                }
            }
        }
    }

    // 6. All files coming with -i option
    for image_source in &config.image_sources {
        match image_source {
            ImageSource::File(image_path) => {
                log_info(&format!(
                    "Adding image file to ordered content: {image_path}"
                ));
                config
                    .ordered_content
                    .push(ContentSource::ImageFile(image_path.clone()));
            }
        }
    }

    // Build final text for backwards compatibility
    Ok(build_legacy_input_text(&config.ordered_content))
}

fn build_legacy_input_text(ordered_content: &[ContentSource]) -> String {
    let mut input_text = String::new();

    for content in ordered_content {
        match content {
            ContentSource::CommandLinePrompt(prompt) => {
                if !input_text.is_empty() {
                    input_text.push_str("\n\n");
                }
                writeln!(input_text, "=== Prompt ===").unwrap();
                input_text.push_str(prompt);
            }
            ContentSource::ClipboardText(text) => {
                if !input_text.is_empty() {
                    input_text.push_str("\n\n");
                }
                writeln!(input_text, "=== Content from: clipboard ===").unwrap();
                input_text.push_str(text);
            }
            ContentSource::StdinText(text) => {
                if !input_text.is_empty() {
                    input_text.push_str("\n\n");
                }
                writeln!(input_text, "=== Content from: stdin ===").unwrap();
                input_text.push_str(text);
            }
            ContentSource::TextFile(file_path, content) => {
                if !input_text.is_empty() {
                    input_text.push_str("\n\n");
                }
                writeln!(input_text, "=== Content from: {file_path} ===").unwrap();
                input_text.push_str(content);
                if !content.ends_with('\n') {
                    input_text.push('\n');
                }
            }
            ContentSource::ConversationHistory(history) => {
                if !input_text.is_empty() {
                    input_text.push_str("\n\n");
                }
                writeln!(input_text, "=== Previous conversation ===").unwrap();
                input_text.push_str(history);
            }
            ContentSource::RoleDefinition(name, content, is_task) => {
                if !input_text.is_empty() {
                    input_text.push_str("\n\n");
                }
                if *is_task {
                    writeln!(input_text, "=== Task: {name} ===").unwrap();
                } else {
                    writeln!(input_text, "=== Role: {name} ===").unwrap();
                }
                input_text.push_str(content);
                if !content.ends_with('\n') {
                    input_text.push('\n');
                }
            }
            // Audio, image files, and clipboard images are handled in multimodal request
            ContentSource::AudioRecording(_)
            | ContentSource::ImageFile(_)
            | ContentSource::ClipboardImage => {
                // These don't contribute to text content
            }
        }
    }

    input_text
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
                validate_media_file(image_path)
                    .with_context(|| format!("Failed to validate image file: {image_path}"))?;
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
            record_audio: false,
            roles: vec![],
            ordered_content: Vec::new(),
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
            record_audio: false,
            roles: vec![],
            ordered_content: Vec::new(),
        };

        let result = get_input_text(&mut config, None).unwrap();
        assert_eq!(result, "=== Prompt ===\nTest prompt");
    }

    #[test]
    fn test_get_input_text_with_prompt_override() {
        let mut config = Config {
            prompt: "Original prompt".to_string(),
            use_clipboard_input: false,
            image_sources: vec![],
            text_files: vec![],
            output_mode: OutputMode::Stdout,
            resume_conversation: None,
            resume_last: false,
            list_conversations: None,
            show_conversation: None,
            model: "test-model".to_string(),
            record_audio: false,
            roles: vec![],
            ordered_content: Vec::new(),
        };

        let result = get_input_text(&mut config, Some("Override prompt")).unwrap();
        assert_eq!(result, "=== Prompt ===\nOverride prompt");
        assert_eq!(config.ordered_content.len(), 1);
    }

    #[test]
    fn test_build_legacy_input_text_multiple_sources() {
        let ordered_content = vec![
            ContentSource::CommandLinePrompt("Main prompt".to_string()),
            ContentSource::ClipboardText("Clipboard content".to_string()),
            ContentSource::StdinText("Stdin content".to_string()),
        ];

        let result = build_legacy_input_text(&ordered_content);

        assert!(result.contains("Main prompt"));
        assert!(result.contains("=== Content from: clipboard ==="));
        assert!(result.contains("Clipboard content"));
        assert!(result.contains("=== Content from: stdin ==="));
        assert!(result.contains("Stdin content"));
    }

    #[test]
    fn test_build_legacy_input_text_with_images_excluded() {
        let ordered_content = vec![
            ContentSource::CommandLinePrompt("Text prompt".to_string()),
            ContentSource::ImageFile("image.jpg".to_string()),
            ContentSource::ClipboardImage,
        ];

        let result = build_legacy_input_text(&ordered_content);

        assert!(result.contains("Text prompt"));
        assert!(!result.contains("image.jpg"));
        assert!(!result.contains("clipboard"));
    }

    #[test]
    fn test_build_legacy_input_text_text_file_with_newline() {
        let content_with_newline = "File content\n";
        let content_without_newline = "File content";

        let ordered_with = vec![ContentSource::TextFile(
            "file1.txt".to_string(),
            content_with_newline.to_string(),
        )];
        let ordered_without = vec![ContentSource::TextFile(
            "file2.txt".to_string(),
            content_without_newline.to_string(),
        )];

        let result_with = build_legacy_input_text(&ordered_with);
        let result_without = build_legacy_input_text(&ordered_without);

        // Both should end with newline
        assert!(result_with.contains("File content\n"));
        assert!(result_without.contains("File content\n"));
    }

    #[test]
    fn test_validate_image_sources_empty() {
        let config = Config {
            prompt: "Test".to_string(),
            use_clipboard_input: false,
            image_sources: vec![],
            text_files: vec![],
            output_mode: OutputMode::Stdout,
            resume_conversation: None,
            resume_last: false,
            list_conversations: None,
            show_conversation: None,
            model: "test-model".to_string(),
            record_audio: false,
            roles: vec![],
            ordered_content: Vec::new(),
        };

        let result = validate_image_sources(&config);
        assert!(result.is_ok());
    }
}
