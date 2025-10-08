use anyhow::{Context, Result};
use chrono::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};
use tts::Tts;

use crate::browser_preview::{open_markdown_preview, FooterMetadata};
use crate::cli::{Config, ContentSource, OutputMode};
use crate::clipboard::write_clipboard;
use crate::conversation::{Conversation, TokenUsage};
use crate::logging::{log_error, log_info, log_trace};

#[cfg(not(target_os = "macos"))]
use notify_rust::Notification;

fn wrap_text(text: &str, width: usize) -> String {
    // First pass: merge lines ending with '•' or number followed by '.'
    let mut merged_lines = Vec::new();
    let lines: Vec<&str> = text.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_end();

        // Check if line ends with '•' or a digit followed by '.'
        let should_merge = if trimmed.ends_with('•') {
            true
        } else if trimmed.ends_with('.') {
            trimmed
                .chars()
                .rev()
                .nth(1)
                .is_some_and(|c| c.is_ascii_digit())
        } else {
            false
        };

        if should_merge && i + 1 < lines.len() {
            // Remove the '•' or keep the number+period, then concatenate with next line
            let content = if trimmed.ends_with('•') {
                trimmed.trim_end_matches('•')
            } else {
                trimmed
            };
            merged_lines.push(format!("{} {}", content, lines[i + 1]));
            i += 2; // Skip the next line as we've merged it
        } else {
            merged_lines.push(line.to_string());
            i += 1;
        }
    }

    // Second pass: wrap the merged lines and add spacing when indentation decreases
    let mut result = Vec::new();
    let mut prev_indent: Option<usize> = None;
    let mut prev_was_empty = false;

    for line in merged_lines.iter() {
        let is_empty = line.trim().is_empty();
        let curr_indent = line.chars().position(|c| c.is_alphanumeric()).unwrap_or(0);

        // Add newline if indentation decreased and previous line wasn't empty
        if let Some(prev) = prev_indent {
            if !prev_was_empty && !is_empty && curr_indent < prev {
                result.push(String::new());
            }
        }

        // Create indentation string matching the position
        let indent = " ".repeat(curr_indent);

        // Configure textwrap options with subsequent indent
        let options = textwrap::Options::new(width).subsequent_indent(&indent);

        result.push(textwrap::fill(line, &options));

        prev_indent = Some(curr_indent);
        prev_was_empty = is_empty;
    }

    result.join("\n")
}

pub fn get_outputs_dir() -> Result<PathBuf> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    Ok(home_dir.join(".gia").join("outputs"))
}

/// Show a system notification for audio recording completion
fn show_audio_completion_notification(output_mode: &OutputMode) {
    let message = match output_mode {
        OutputMode::Clipboard => "Recording complete! Result copied to clipboard.",
        OutputMode::TempFileWithPreview => "Recording complete! Preview opened in browser.",
        OutputMode::Stdout => "Recording complete! Check your terminal.",
        OutputMode::Tts(_) => "Recording complete! Playing audio response.",
    };

    #[cfg(target_os = "macos")]
    {
        // On macOS, use osascript to show notification
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg(format!(
                "display notification \"{}\" with title \"GIA Audio Recording\"",
                message
            ))
            .output();
        log_info("Showed macOS notification for audio recording completion");
    }

    #[cfg(not(target_os = "macos"))]
    {
        // On Windows and Linux, use notify-rust
        let _ = Notification::new()
            .summary("GIA Audio Recording")
            .body(message)
            .icon("microphone")
            .show();
        log_info("Showed system notification for audio recording completion");
    }
}

fn build_footer_metadata(config: &Config, token_usage: Option<TokenUsage>) -> FooterMetadata {
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
    let mut roles = Vec::new();
    let mut tasks = Vec::new();

    for content in &config.ordered_content {
        match content {
            ContentSource::RoleDefinition(name, _, is_task) => {
                if *is_task {
                    tasks.push(name.clone());
                } else {
                    roles.push(name.clone());
                }
            }
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
        roles,
        tasks,
        prompt: config.prompt.clone(),
        token_usage,
    }
}

pub fn output_text_with_usage(
    text: &str,
    config: &Config,
    token_usage: Option<TokenUsage>,
    conversation_id: &str,
) -> Result<()> {
    // Check if audio recording was used
    let has_audio_recording = config
        .ordered_content
        .iter()
        .any(|content| matches!(content, ContentSource::AudioRecording(_)));

    let result = match &config.output_mode {
        OutputMode::TempFileWithPreview => {
            log_info("Writing response to output file, copying file path to clipboard, and opening browser preview");

            // Get outputs directory and create it if it doesn't exist
            let outputs_dir = get_outputs_dir()?;
            if !outputs_dir.exists() {
                fs::create_dir_all(&outputs_dir).context("Failed to create outputs directory")?;
                log_info(&format!("Created outputs directory: {outputs_dir:?}"));
            }

            // Create output file with conversation ID + timestamp
            let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
            let filename = format!("{}_{}.md", conversation_id, timestamp);
            let output_path = outputs_dir.join(filename);

            // Write content to output file
            fs::write(&output_path, text)?;

            // Copy file path to clipboard
            let file_path_str = output_path.to_string_lossy();
            write_clipboard(&file_path_str)?;

            // Build footer metadata
            let metadata = build_footer_metadata(config, token_usage);

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
            println!("{wrapped_text}");
            Ok(())
        }
        OutputMode::Tts(lang) => {
            log_info(&format!(
                "Speaking response using TTS with language: {lang}"
            ));
            let plain_text = markdown_to_text::convert(text);
            let plain_text = plain_text.replace('\t', "  ");

            // First output to stdout
            let wrapped_text = wrap_text(&plain_text, 100);
            println!("{wrapped_text}");

            // Then speak using TTS
            speak_and_wait(&plain_text, lang)
        }
    };

    // Show notification only if audio recording was used AND output is to clipboard
    if has_audio_recording && matches!(config.output_mode, OutputMode::Clipboard) {
        show_audio_completion_notification(&config.output_mode);
    }

    result
}

// Function removed - now in conversation.rs as Conversation::extract_prompt_section()

fn setup_tts_voice(tts: &mut Tts, lang: &str) -> Result<()> {
    let voices = tts.voices()?;
    let target_voice = voices.iter().find(|v| {
        v.language()
            .to_lowercase()
            .starts_with(&lang.to_lowercase())
            || v.language()
                .to_lowercase()
                .starts_with(&lang[..2].to_lowercase())
    });

    if let Some(voice) = target_voice {
        tts.set_voice(voice)?;
        log_info(&format!(
            "Using voice: {} ({})",
            voice.name(),
            voice.language()
        ));
    } else {
        log_info(&format!(
            "No voice found for language {lang}, using default"
        ));
    }

    Ok(())
}

/// Speak text using TTS and wait for completion
fn speak_and_wait(text: &str, lang: &str) -> Result<()> {
    let mut tts = Tts::default()?;
    setup_tts_voice(&mut tts, lang)?;
    tts.speak(text, true)?;

    // Small delay to let speech start
    std::thread::sleep(std::time::Duration::from_millis(200));

    log_info("Waiting for speech to complete...");
    // Wait for speech to complete
    while tts.is_speaking()? {
        log_trace("Still speaking...");
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    log_info("Speech complete");

    Ok(())
}

pub fn speak_conversation(conversation: &Conversation, lang: &str) -> Result<()> {
    log_info("Extracting conversation content for TTS");

    let mut content_to_speak = String::new();

    for message in &conversation.messages {
        match message.role.as_str() {
            "User" => {
                // Only include command line prompts (skip resources)
                let prompt_text = Conversation::extract_prompt_section(message);
                if !prompt_text.is_empty() {
                    content_to_speak.push_str("User: ");
                    content_to_speak.push_str(&prompt_text);
                    content_to_speak.push_str("\n\n");
                }
            }
            "Assistant" => {
                // Convert markdown to plain text and add to content
                let text_content = Conversation::extract_text_content(message);
                let plain_text = markdown_to_text::convert(&text_content);
                let plain_text = plain_text.replace('\t', "  ");
                content_to_speak.push_str("Assistant: ");
                content_to_speak.push_str(&plain_text);
                content_to_speak.push_str("\n\n");
            }
            _ => {
                // Ignore System/Tool messages
            }
        }
    }

    if content_to_speak.is_empty() {
        log_info("No content to speak");
        return Ok(());
    }

    // Output to stdout first
    let wrapped_text = wrap_text(&content_to_speak, 100);
    println!("{wrapped_text}");

    // Then speak using TTS
    speak_and_wait(&content_to_speak, lang)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap_text_basic() {
        let text = "This is a simple line that should be wrapped at the specified width limit.";
        let wrapped = wrap_text(text, 30);
        let lines: Vec<&str> = wrapped.lines().collect();

        // Should be wrapped into multiple lines
        assert!(lines.len() > 1);
        for line in lines {
            assert!(line.len() <= 30);
        }
    }

    #[test]
    fn test_wrap_text_bullet_merge() {
        let text = "Item 1•\nContinuation of item 1";
        let wrapped = wrap_text(text, 100);

        // Should merge lines ending with bullet
        assert!(!wrapped.contains("•\n"));
        assert!(wrapped.contains("Item 1 Continuation of item 1"));
    }

    #[test]
    fn test_wrap_text_numbered_list_merge() {
        let text = "1.\nFirst item content";
        let wrapped = wrap_text(text, 100);

        // Should merge numbered list with content
        assert!(wrapped.contains("1. First item content"));
    }

    #[test]
    fn test_wrap_text_preserves_indentation() {
        let text = "    Indented line that needs wrapping at some point in the future";
        let wrapped = wrap_text(text, 30);
        let lines: Vec<&str> = wrapped.lines().collect();

        // First line should have indentation
        assert!(lines[0].starts_with("    "));

        // Continuation lines should also be indented
        if lines.len() > 1 {
            assert!(lines[1].starts_with("    "));
        }
    }

    #[test]
    fn test_get_outputs_dir() {
        let result = get_outputs_dir();
        assert!(result.is_ok());

        let path = result.unwrap();
        assert!(path.to_string_lossy().contains(".gia"));
        assert!(path.to_string_lossy().contains("outputs"));
    }

    #[test]
    fn test_build_footer_metadata() {
        let config = Config {
            prompt: "Test prompt".to_string(),
            use_clipboard_input: true,
            text_files: vec!["file.txt".to_string()],
            output_mode: OutputMode::Stdout,
            resume_conversation: None,
            resume_last: false,
            list_conversations: None,
            show_conversation: None,
            model: "openai::gpt-4".to_string(),
            record_audio: false,
            roles: vec!["assistant".to_string()],
            ordered_content: vec![
                ContentSource::ImageFile("test.jpg".to_string()),
                ContentSource::TextFile("file.txt".to_string(), "content".to_string()),
                ContentSource::ClipboardText("clipboard".to_string()),
            ],
        };

        let metadata = build_footer_metadata(&config, None);

        assert_eq!(metadata.provider_name, "openai");
        assert_eq!(metadata.model_name, "gpt-4");
        assert_eq!(metadata.prompt, "Test prompt");
        assert!(metadata.has_clipboard);
        assert_eq!(metadata.image_files, vec!["test.jpg"]);
        assert_eq!(metadata.text_files, vec!["file.txt"]);
    }

    #[test]
    fn test_build_footer_metadata_with_provider_prefix() {
        let config = Config {
            prompt: "Test prompt".to_string(),
            use_clipboard_input: false,
            text_files: vec![],
            output_mode: OutputMode::Stdout,
            resume_conversation: None,
            resume_last: false,
            list_conversations: None,
            show_conversation: None,
            model: "openai::gpt-4".to_string(),
            record_audio: false,
            roles: vec![],
            ordered_content: Vec::new(),
        };

        let metadata = build_footer_metadata(&config, None);

        assert_eq!(metadata.provider_name, "openai");
        assert_eq!(metadata.model_name, "gpt-4");
    }
}
