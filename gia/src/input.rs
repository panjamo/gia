use anyhow::{Context, Result};
use chardetng::EncodingDetector;
use std::fs;
use std::io::{self, Read};
use std::path::Path;

use crate::audio::record_audio;
use crate::cli::{Config, ContentSource, OutputMode};
use crate::clipboard::{has_clipboard_image, read_clipboard, write_clipboard};
use crate::constants::MEDIA_EXTENSIONS;

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

    // First try to read as bytes
    let bytes = fs::read(file_path).with_context(|| format!("Failed to read file: {file_path}"))?;

    // Try to decode as UTF-8 first
    match String::from_utf8(bytes.clone()) {
        Ok(content) => {
            log_info(&format!(
                "Read {} characters from file (UTF-8): {}",
                content.len(),
                file_path
            ));
            Ok(content)
        }
        Err(_) => {
            // If UTF-8 fails, use encoding detection
            let mut detector = EncodingDetector::new();
            detector.feed(&bytes, true);
            let encoding = detector.guess(None, true);

            let (content, _, had_errors) = encoding.decode(&bytes);
            if had_errors {
                log_debug(&format!(
                    "Encoding detection had errors for file: {file_path}"
                ));
            }

            log_info(&format!(
                "Read {} characters from file ({}): {}",
                content.len(),
                encoding.name(),
                file_path
            ));
            Ok(content.into_owned())
        }
    }
}

#[derive(Debug, PartialEq)]
enum FileType {
    Media,
    Text,
    Binary,
}

/// Check if a file is a media file based on its extension
fn is_media_file_by_extension(path: &Path) -> bool {
    if let Some(ext) = path.extension()
        && let Some(ext_str) = ext.to_str()
    {
        return MEDIA_EXTENSIONS.contains(&ext_str.to_lowercase().as_str());
    }
    false
}

/// Detect whether a file is a media file, text file, or binary file
fn detect_file_type(path: &Path) -> FileType {
    // First check if it's a known media file by extension
    if is_media_file_by_extension(path) {
        return FileType::Media;
    }

    // Try to analyze file content to determine if it's text or binary
    match analyze_file_content(path) {
        Ok(true) => FileType::Text,
        Ok(false) => FileType::Binary,
        Err(_) => {
            // If we can't read the file, assume it's binary to be safe
            log_debug(&format!(
                "Could not analyze file content, treating as binary: {}",
                path.display()
            ));
            FileType::Binary
        }
    }
}

/// Analyze file content to determine if it's likely a text file
fn analyze_file_content(path: &Path) -> Result<bool> {
    // Read first 8KB for analysis (enough to detect most text files)
    const SAMPLE_SIZE: usize = 8192;

    let file = fs::File::open(path)?;
    let mut buffer = vec![0u8; SAMPLE_SIZE];
    let mut reader = std::io::BufReader::new(file);
    let bytes_read = reader.read(&mut buffer)?;

    if bytes_read == 0 {
        // Empty file, treat as text
        return Ok(true);
    }

    buffer.truncate(bytes_read);
    Ok(is_text_content(&buffer))
}

/// Determine if the given bytes represent text content
fn is_text_content(data: &[u8]) -> bool {
    // Check for UTF-8/UTF-16 BOM (Byte Order Mark)
    if has_text_bom(data) {
        return true;
    }

    // Check for null bytes (strong indicator of binary content)
    if data.contains(&0) {
        return false;
    }

    // Try UTF-8 decoding first
    if std::str::from_utf8(data).is_ok() {
        return true;
    }

    // For non-UTF-8, be more conservative about binary detection
    // Count different types of characters
    let mut printable_count = 0;
    let mut high_byte_count = 0;
    let mut control_count = 0;

    for &byte in data {
        match byte {
            // ASCII printable + common whitespace
            0x09 | 0x0A | 0x0D | 0x20..=0x7E => printable_count += 1,
            // High bytes (could be text encoding, but suspicious in sequences)
            0x80..=0xFF => high_byte_count += 1,
            // Control characters
            _ => control_count += 1,
        }
    }

    let total = data.len();
    let printable_ratio = printable_count as f64 / total as f64;
    let high_byte_ratio = high_byte_count as f64 / total as f64;

    // If mostly ASCII printable, it's text
    if printable_ratio > 0.8 {
        return true;
    }

    // If high ratio of high bytes (>50%), likely binary
    if high_byte_ratio > 0.5 {
        return false;
    }

    // For mixed content, need decent printable ratio and not too many control chars
    printable_ratio > 0.6 && (control_count as f64 / total as f64) < 0.1
}

/// Check for UTF-8 or UTF-16 Byte Order Mark
fn has_text_bom(data: &[u8]) -> bool {
    if data.len() >= 3 {
        // UTF-8 BOM: EF BB BF
        if data[0] == 0xEF && data[1] == 0xBB && data[2] == 0xBF {
            return true;
        }
    }

    if data.len() >= 2 {
        // UTF-16 BOM: FF FE or FE FF
        if (data[0] == 0xFF && data[1] == 0xFE) || (data[0] == 0xFE && data[1] == 0xFF) {
            return true;
        }
    }

    false
}

/// Recursively collect all regular files from a directory or return the single file if it's not a directory
pub fn collect_files_recursive(path: &str) -> Result<Vec<String>> {
    let path_obj = Path::new(path);

    if !path_obj.exists() {
        return Err(anyhow::anyhow!("Path does not exist: {}", path));
    }

    if path_obj.is_file() {
        // If it's a file, return it as-is
        return Ok(vec![path.to_string()]);
    }

    if path_obj.is_dir() {
        // If it's a directory, recursively collect all files
        let mut files = Vec::new();
        collect_files_from_dir(path_obj, &mut files)?;
        files.sort(); // Sort for consistent ordering
        return Ok(files);
    }

    Err(anyhow::anyhow!(
        "Path is neither a file nor directory: {}",
        path
    ))
}

/// Helper function to recursively collect files from a directory
fn collect_files_from_dir(dir: &Path, files: &mut Vec<String>) -> Result<()> {
    let entries = fs::read_dir(dir)
        .with_context(|| format!("Failed to read directory: {}", dir.display()))?;

    for entry in entries {
        let entry = entry
            .with_context(|| format!("Failed to read directory entry in: {}", dir.display()))?;
        let path = entry.path();

        if path.is_file() {
            // Add the file to our collection
            if let Some(path_str) = path.to_str() {
                files.push(path_str.to_string());
            } else {
                log_debug(&format!(
                    "Skipping file with invalid UTF-8 path: {:?}",
                    path
                ));
            }
        } else if path.is_dir() {
            // Recursively process subdirectories
            collect_files_from_dir(&path, files)?;
        }
        // Skip symlinks, device files, etc.
    }

    Ok(())
}

pub fn get_input_text(config: &mut Config, prompt_override: Option<&str>) -> Result<()> {
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
        match record_audio(config.audio_device.as_deref()) {
            Ok(audio_path) => {
                log_info(&format!("Audio recorded to: {audio_path}"));
                config
                    .ordered_content
                    .push(ContentSource::AudioRecording(audio_path));

                // If no command line prompt provided, use default audio prompt
                if prompt_to_use.is_empty() {
                    let default_audio_prompt = "Your instructions are in prompt.opus";
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

                // Clear clipboard if output mode is clipboard
                if matches!(config.output_mode, OutputMode::Clipboard) {
                    log_info("Clearing clipboard due to cancelled audio recording");
                    if let Err(clear_err) = write_clipboard("") {
                        log_debug(&format!("Failed to clear clipboard: {clear_err}"));
                    }
                }

                // Return error to stop execution
                return Err(e);
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

    // 5. All files coming with -f option (with recursive directory support)
    if !config.text_files.is_empty() {
        log_info(&format!(
            "Processing {} file path(s)",
            config.text_files.len()
        ));

        for file_path in &config.text_files {
            // Collect all files (handles both files and directories recursively)
            match collect_files_recursive(file_path) {
                Ok(collected_files) => {
                    log_info(&format!(
                        "Collected {} file(s) from path: {}",
                        collected_files.len(),
                        file_path
                    ));

                    for actual_file_path in collected_files {
                        let path = Path::new(&actual_file_path);

                        match detect_file_type(path) {
                            FileType::Media => {
                                // Handle as media file
                                log_info(&format!(
                                    "Auto-detected media file, adding as image: {actual_file_path}"
                                ));
                                config
                                    .ordered_content
                                    .push(ContentSource::ImageFile(actual_file_path));
                            }
                            FileType::Text => {
                                // Handle as text file
                                match read_text_file(&actual_file_path) {
                                    Ok(file_content) => {
                                        if !file_content.trim().is_empty() {
                                            log_info(&format!(
                                                "Auto-detected text file, adding to ordered content: {actual_file_path}"
                                            ));
                                            config.ordered_content.push(ContentSource::TextFile(
                                                actual_file_path,
                                                file_content,
                                            ));
                                        }
                                    }
                                    Err(e) => {
                                        log_debug(&format!(
                                            "Failed to read text file {actual_file_path}: {e}"
                                        ));
                                        eprintln!(
                                            "Warning: Failed to read text file '{actual_file_path}': {e}"
                                        );
                                    }
                                }
                            }
                            FileType::Binary => {
                                // Skip binary files with a warning
                                log_info(&format!(
                                    "Skipping binary file (not media or text): {actual_file_path}"
                                ));
                                eprintln!(
                                    "Warning: Skipping binary file '{}' (not a known media type or text file)",
                                    actual_file_path
                                );
                            }
                        }
                    }
                }
                Err(e) => {
                    log_debug(&format!(
                        "Failed to collect files from path {file_path}: {e}"
                    ));
                    eprintln!("Warning: Failed to process path '{file_path}': {e}");
                }
            }
        }
    }

    // Media files are now handled automatically in the -f option processing above

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
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Failed to read file")
        );
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
            spinner: false,
            audio_device: None,
            list_audio_devices: false,
            no_save: false,
        };

        get_input_text(&mut config, None).unwrap();

        // Verify ordered_content has the expected items
        assert_eq!(config.ordered_content.len(), 3); // prompt + 2 files
        match &config.ordered_content[0] {
            ContentSource::CommandLinePrompt(p) => assert_eq!(p, "Test prompt"),
            _ => panic!("Expected CommandLinePrompt"),
        }
        match &config.ordered_content[1] {
            ContentSource::TextFile(_, c) => assert_eq!(c, content1),
            _ => panic!("Expected TextFile"),
        }
        match &config.ordered_content[2] {
            ContentSource::TextFile(_, c) => assert_eq!(c, content2),
            _ => panic!("Expected TextFile"),
        }
    }

    #[test]
    fn test_collect_files_recursive_single_file() {
        let temp_file = NamedTempFile::new().unwrap();
        let file_path = temp_file.path().to_str().unwrap();

        let result = collect_files_recursive(file_path).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0], file_path);
    }

    #[test]
    fn test_collect_files_recursive_directory() {
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let dir_path = temp_dir.path();

        // Create test files
        let file1 = dir_path.join("file1.txt");
        let file2 = dir_path.join("file2.txt");
        let subdir = dir_path.join("subdir");
        fs::create_dir(&subdir).unwrap();
        let file3 = subdir.join("file3.txt");

        fs::write(&file1, "content1").unwrap();
        fs::write(&file2, "content2").unwrap();
        fs::write(&file3, "content3").unwrap();

        let result = collect_files_recursive(dir_path.to_str().unwrap()).unwrap();
        assert_eq!(result.len(), 3);

        // Files should be sorted
        let mut expected = vec![
            file1.to_str().unwrap().to_string(),
            file2.to_str().unwrap().to_string(),
            file3.to_str().unwrap().to_string(),
        ];
        expected.sort();

        assert_eq!(result, expected);
    }

    #[test]
    fn test_collect_files_recursive_nonexistent() {
        let result = collect_files_recursive("nonexistent_path");
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Path does not exist")
        );
    }

    #[test]
    fn test_get_input_text_empty_files_list() {
        let mut config = Config {
            prompt: "".to_string(),
            use_clipboard_input: false,
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
            spinner: false,
            audio_device: None,
            list_audio_devices: false,
            no_save: false,
        };

        let result = get_input_text(&mut config, None);
        assert!(result.is_ok());

        // Should have no content since files list is empty
        assert!(config.ordered_content.is_empty());
    }

    #[test]
    fn test_get_input_text_with_prompt_override() {
        let mut config = Config {
            prompt: "Original prompt".to_string(),
            use_clipboard_input: false,
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
            spinner: false,
            audio_device: None,
            list_audio_devices: false,
            no_save: false,
        };

        let result = get_input_text(&mut config, Some("Override prompt"));
        assert!(result.is_ok());

        // Should have 1 content item with the override prompt
        assert_eq!(config.ordered_content.len(), 1);
        match &config.ordered_content[0] {
            ContentSource::CommandLinePrompt(p) => assert_eq!(p, "Override prompt"),
            _ => panic!("Expected CommandLinePrompt"),
        }
    }

    #[test]
    fn test_is_media_file_by_extension() {
        use std::path::Path;

        // Test image files
        assert!(is_media_file_by_extension(Path::new("test.jpg")));
        assert!(is_media_file_by_extension(Path::new("test.jpeg")));
        assert!(is_media_file_by_extension(Path::new("test.png")));
        assert!(is_media_file_by_extension(Path::new("test.webp")));
        assert!(is_media_file_by_extension(Path::new("test.heic")));
        assert!(is_media_file_by_extension(Path::new("test.pdf")));

        // Test audio files
        assert!(is_media_file_by_extension(Path::new("test.ogg")));
        assert!(is_media_file_by_extension(Path::new("test.opus")));
        assert!(is_media_file_by_extension(Path::new("test.mp3")));
        assert!(is_media_file_by_extension(Path::new("test.m4a")));

        // Test video files
        assert!(is_media_file_by_extension(Path::new("test.mp4")));

        // Test case insensitive
        assert!(is_media_file_by_extension(Path::new("test.JPG")));
        assert!(is_media_file_by_extension(Path::new("test.PNG")));
        assert!(is_media_file_by_extension(Path::new("test.MP4")));

        // Test non-media files
        assert!(!is_media_file_by_extension(Path::new("test.txt")));
        assert!(!is_media_file_by_extension(Path::new("test.rs")));
        assert!(!is_media_file_by_extension(Path::new("test.md")));
        assert!(!is_media_file_by_extension(Path::new("test.json")));
        assert!(!is_media_file_by_extension(Path::new("test.xml")));

        // Test files without extension
        assert!(!is_media_file_by_extension(Path::new("test")));
        assert!(!is_media_file_by_extension(Path::new("README")));
    }

    #[test]
    fn test_is_text_content() {
        // Test UTF-8 content
        assert!(is_text_content(b"Hello, world!"));
        assert!(is_text_content("Hello, 世界!".as_bytes()));

        // Test UTF-8 BOM
        let utf8_bom = [0xEF, 0xBB, 0xBF, b'H', b'e', b'l', b'l', b'o'];
        assert!(is_text_content(&utf8_bom));

        // Test UTF-16 BOM
        let utf16_le_bom = [0xFF, 0xFE, b'H', 0x00, b'i', 0x00];
        assert!(is_text_content(&utf16_le_bom));

        // Test common text content
        assert!(is_text_content(
            b"This is a text file\nwith multiple lines\r\n"
        ));
        assert!(is_text_content(b"{\n  \"key\": \"value\"\n}"));

        // Test binary content (contains null bytes)
        assert!(!is_text_content(&[0x50, 0x4B, 0x03, 0x04, 0x00, 0x00])); // ZIP header
        assert!(!is_text_content(&[0xFF, 0xD8, 0xFF, 0xE0])); // JPEG header

        // Test mixed content (should be detected as binary due to null bytes)
        assert!(!is_text_content(&[
            b'H', b'e', b'l', b'l', b'o', 0x00, b'w', b'o', b'r', b'l', b'd'
        ]));

        // Test high ratio of high bytes (should be binary)
        let binary_like = vec![0x80; 100]; // All high bytes
        assert!(!is_text_content(&binary_like));

        // Test mixed content with some high bytes (should still be text)
        let mixed_content = [b'H', b'e', b'l', b'l', b'o', 0xE9, b'!', b'\n']; // "Helloé!\n"
        assert!(is_text_content(&mixed_content));
    }

    #[test]
    fn test_has_text_bom() {
        // UTF-8 BOM
        assert!(has_text_bom(&[0xEF, 0xBB, 0xBF]));
        assert!(has_text_bom(&[0xEF, 0xBB, 0xBF, b'H', b'i']));

        // UTF-16 LE BOM
        assert!(has_text_bom(&[0xFF, 0xFE]));
        assert!(has_text_bom(&[0xFF, 0xFE, b'H', 0x00]));

        // UTF-16 BE BOM
        assert!(has_text_bom(&[0xFE, 0xFF]));
        assert!(has_text_bom(&[0xFE, 0xFF, 0x00, b'H']));

        // No BOM
        assert!(!has_text_bom(b"Hello, world!"));
        assert!(!has_text_bom(&[0xEF, 0xBB])); // Incomplete UTF-8 BOM
        assert!(!has_text_bom(&[0xFF])); // Incomplete UTF-16 BOM
        assert!(!has_text_bom(&[])); // Empty
    }

    #[test]
    fn test_get_input_text_with_mixed_files() {
        use tempfile::NamedTempFile;

        let temp_text_file = NamedTempFile::new().unwrap();
        let temp_image_file = NamedTempFile::with_suffix(".jpg").unwrap();
        let temp_audio_file = NamedTempFile::with_suffix(".mp3").unwrap();

        let text_content = "This is a text file content";
        fs::write(temp_text_file.path(), text_content).unwrap();
        fs::write(temp_image_file.path(), b"fake image data").unwrap();
        fs::write(temp_audio_file.path(), b"fake audio data").unwrap();

        let mut config = Config {
            prompt: "Test prompt".to_string(),
            use_clipboard_input: false,
            text_files: vec![
                temp_text_file.path().to_str().unwrap().to_string(),
                temp_image_file.path().to_str().unwrap().to_string(),
                temp_audio_file.path().to_str().unwrap().to_string(),
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
            spinner: false,
            audio_device: None,
            list_audio_devices: false,
            no_save: false,
        };

        get_input_text(&mut config, None).unwrap();

        // Should have prompt + 1 text file + 2 media files
        assert_eq!(config.ordered_content.len(), 4);

        // Check prompt
        match &config.ordered_content[0] {
            ContentSource::CommandLinePrompt(p) => assert_eq!(p, "Test prompt"),
            _ => panic!("Expected CommandLinePrompt"),
        }

        // Check text file
        let mut found_text_file = false;
        let mut found_image_file = false;
        let mut found_audio_file = false;

        for content in &config.ordered_content[1..] {
            match content {
                ContentSource::TextFile(path, content) => {
                    if path == temp_text_file.path().to_str().unwrap() {
                        assert_eq!(content, text_content);
                        found_text_file = true;
                    }
                }
                ContentSource::ImageFile(path) => {
                    if path == temp_image_file.path().to_str().unwrap() {
                        found_image_file = true;
                    } else if path == temp_audio_file.path().to_str().unwrap() {
                        found_audio_file = true;
                    }
                }
                _ => {}
            }
        }

        assert!(found_text_file, "Text file should be processed as text");
        assert!(found_image_file, "Image file should be processed as media");
        assert!(found_audio_file, "Audio file should be processed as media");
    }

    #[test]
    fn test_get_input_text_with_binary_files() {
        use tempfile::NamedTempFile;

        let temp_text_file = NamedTempFile::new().unwrap();
        let temp_binary_file = NamedTempFile::with_suffix(".bin").unwrap();

        let text_content = "This is a text file content";
        let binary_content = vec![0x00, 0x01, 0x02, 0x03, 0xFF, 0xFE, 0xFD]; // Clear binary data

        fs::write(temp_text_file.path(), text_content).unwrap();
        fs::write(temp_binary_file.path(), &binary_content).unwrap();

        let mut config = Config {
            prompt: "Test prompt".to_string(),
            use_clipboard_input: false,
            text_files: vec![
                temp_text_file.path().to_str().unwrap().to_string(),
                temp_binary_file.path().to_str().unwrap().to_string(),
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
            spinner: false,
            audio_device: None,
            list_audio_devices: false,
            no_save: false,
        };

        get_input_text(&mut config, None).unwrap();

        // Should have prompt + 1 text file (binary file should be skipped)
        assert_eq!(config.ordered_content.len(), 2);

        // Check prompt
        match &config.ordered_content[0] {
            ContentSource::CommandLinePrompt(p) => assert_eq!(p, "Test prompt"),
            _ => panic!("Expected CommandLinePrompt"),
        }

        // Check that only the text file was processed
        match &config.ordered_content[1] {
            ContentSource::TextFile(path, content) => {
                assert_eq!(path, temp_text_file.path().to_str().unwrap());
                assert_eq!(content, text_content);
            }
            _ => panic!("Expected TextFile"),
        }
    }

    #[test]
    fn test_get_input_text_with_directory_containing_mixed_files() {
        use std::fs;
        use tempfile::TempDir;

        let temp_dir = TempDir::new().unwrap();
        let text_file = temp_dir.path().join("document.txt");
        let image_file = temp_dir.path().join("photo.png");
        let audio_file = temp_dir.path().join("recording.mp3");
        let video_file = temp_dir.path().join("movie.mp4");

        let text_content = "Document content";
        fs::write(&text_file, text_content).unwrap();
        fs::write(&image_file, b"fake png data").unwrap();
        fs::write(&audio_file, b"fake mp3 data").unwrap();
        fs::write(&video_file, b"fake mp4 data").unwrap();

        let mut config = Config {
            prompt: "Process this directory".to_string(),
            use_clipboard_input: false,
            text_files: vec![temp_dir.path().to_str().unwrap().to_string()],
            output_mode: OutputMode::Stdout,
            resume_conversation: None,
            resume_last: false,
            list_conversations: None,
            show_conversation: None,
            model: "test-model".to_string(),
            record_audio: false,
            roles: vec![],
            ordered_content: Vec::new(),
            spinner: false,
            audio_device: None,
            list_audio_devices: false,
            no_save: false,
        };

        get_input_text(&mut config, None).unwrap();

        // Should have prompt + 1 text file + 3 media files
        assert_eq!(config.ordered_content.len(), 5);

        // Check prompt
        match &config.ordered_content[0] {
            ContentSource::CommandLinePrompt(p) => assert_eq!(p, "Process this directory"),
            _ => panic!("Expected CommandLinePrompt"),
        }

        // Verify we have exactly 1 text file and 3 media files
        let mut text_files_count = 0;
        let mut media_files_count = 0;

        for content in &config.ordered_content[1..] {
            match content {
                ContentSource::TextFile(_, _) => text_files_count += 1,
                ContentSource::ImageFile(_) => media_files_count += 1,
                _ => {}
            }
        }

        assert_eq!(text_files_count, 1, "Should have exactly 1 text file");
        assert_eq!(media_files_count, 3, "Should have exactly 3 media files");
    }
}
