use crate::logging::log_info;
use clap::{Arg, Command};

#[derive(Debug, Clone)]
pub enum OutputMode {
    Stdout,
    Clipboard,
    TempFileWithPreview,
}

#[derive(Debug, Clone)]
pub enum ImageSource {
    File(String),
    Clipboard,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub prompt: String,
    pub use_clipboard_input: bool,
    pub image_sources: Vec<ImageSource>,
    pub text_files: Vec<String>,
    pub output_mode: OutputMode,
    pub resume_conversation: Option<String>, // None = new, Some("") = latest, Some(id) = specific
    pub resume_last: bool,                   // true = resume latest conversation
    pub list_conversations: Option<usize>, // None = don't list, Some(n) = list top n, Some(0) = list all
    pub show_conversation: Option<String>, // Some(id) = show specific conversation
    pub model: String,
}

impl Config {
    pub fn from_args() -> Self {
        let matches = Self::build_cli().get_matches();

        // Handle verbose help immediately
        if matches.get_flag("verbose-help") {
            Self::handle_verbose_help();
            std::process::exit(0);
        }

        let prompt_parts: Vec<String> = matches
            .get_many::<String>("prompt")
            .unwrap_or_default()
            .cloned()
            .collect();

        let resume_conversation = matches.get_one::<String>("resume").cloned();

        let output_mode = if matches.get_flag("browser-output") {
            OutputMode::TempFileWithPreview
        } else if matches.get_flag("clipboard-output") {
            OutputMode::Clipboard
        } else {
            OutputMode::Stdout
        };

        let image_paths: Vec<String> = matches
            .get_many::<String>("image")
            .unwrap_or_default()
            .cloned()
            .collect();

        // Convert image paths to image sources
        let image_sources: Vec<ImageSource> = image_paths
            .iter()
            .map(|path| ImageSource::File(path.clone()))
            .collect();

        let text_files: Vec<String> = matches
            .get_many::<String>("file")
            .unwrap_or_default()
            .cloned()
            .collect();

        Self {
            prompt: prompt_parts.join(" "),
            use_clipboard_input: matches.get_flag("clipboard-input"),
            image_sources,
            text_files,
            output_mode,
            resume_conversation,
            resume_last: matches.get_flag("resume-last"),
            list_conversations: matches
                .get_one::<String>("list-conversations")
                .map(|s| s.parse::<usize>().unwrap_or(0)),
            show_conversation: matches.get_one::<String>("show-conversation").cloned(),
            model: matches.get_one::<String>("model").unwrap().clone(),
        }
    }

    fn build_cli() -> Command {
        Command::new("gia")
            .version("0.1.0")
            .about("AI CLI tool using Google Gemini API (stdout default)")
            .arg(
                Arg::new("prompt")
                    .help("Prompt text for the AI")
                    .num_args(0..)
                    .required(false),
            )
            .arg(
                Arg::new("clipboard-input")
                    .short('c')
                    .long("clipboard-input")
                    .help("Add clipboard content to prompt")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                Arg::new("image")
                    .short('i')
                    .long("image")
                    .help("Add image file to prompt (can be used multiple times)")
                    .value_name("FILE")
                    .action(clap::ArgAction::Append),
            )
            .arg(
                Arg::new("file")
                    .short('f')
                    .long("file")
                    .help("Add text file content to prompt (can be used multiple times)")
                    .value_name("FILE")
                    .action(clap::ArgAction::Append),
            )
            .arg(
                Arg::new("clipboard-output")
                    .short('o')
                    .long("clipboard-output")
                    .help("Write output to clipboard instead of stdout")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                Arg::new("browser-output")
                    .short('b')
                    .long("browser-output")
                    .help("Write output to temp file, copy file path to clipboard, and open browser preview")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                Arg::new("resume")
                    .short('r')
                    .long("resume")
                    .help("Resume last conversation or specify conversation ID")
                    .value_name("ID")
                    .num_args(0..=1)
                    .default_missing_value("")
                    .action(clap::ArgAction::Set),
            )
            .arg(
                Arg::new("resume-last")
                    .short('R')
                    .help("Resume the very last conversation")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                Arg::new("list-conversations")
                    .short('l')
                    .long("list-conversations")
                    .help("List saved conversations (optionally specify number to limit, empty for all)")
                    .value_name("NUMBER")
                    .num_args(0..=1)
                    .default_missing_value("0")
                    .action(clap::ArgAction::Set),
            )
            .arg(
                Arg::new("show-conversation")
                    .short('s')
                    .long("show-conversation")
                    .help("Show a specific conversation in chat mode (or latest if no ID provided)")
                    .value_name("ID")
                    .num_args(0..=1)
                    .default_missing_value("")
                    .action(clap::ArgAction::Set),
            )
            .arg(
                Arg::new("model")
                    .short('m')
                    .long("model")
                    .help("Specify the Gemini model to use (see https://ai.google.dev/gemini-api/docs/models)")
                    .value_name("MODEL")
                    .default_value("gemini-2.5-flash-lite")
                    .action(clap::ArgAction::Set),
            )
            .arg(
                Arg::new("verbose-help")
                    .long("verbose-help")
                    .help("Open browser with detailed documentation on GitHub")
                    .action(clap::ArgAction::SetTrue),
            )
    }

    pub fn add_clipboard_image(&mut self) {
        log_info("Adding clipboard image to image sources");
        self.image_sources.push(ImageSource::Clipboard);
    }

    fn handle_verbose_help() {
        const GITHUB_README_URL: &str = "https://github.com/panjamo/gia/blob/master/README.md";

        println!("Opening detailed documentation in your browser...");

        if let Err(e) = webbrowser::open(GITHUB_README_URL) {
            eprintln!("Failed to open browser: {e}");
            eprintln!("Please visit: {GITHUB_README_URL}");
        } else {
            println!("Documentation URL: {GITHUB_README_URL}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_image_source_debug() {
        let file_source = ImageSource::File("test.jpg".to_string());
        let clipboard_source = ImageSource::Clipboard;

        // Test that Debug is implemented
        let _file_debug = format!("{file_source:?}");
        let _clipboard_debug = format!("{clipboard_source:?}");
    }

    #[test]
    fn test_config_add_clipboard_image() {
        let mut config = Config {
            prompt: "test".to_string(),
            use_clipboard_input: false,
            image_sources: vec![],
            text_files: vec![],
            output_mode: OutputMode::Stdout,
            resume_conversation: None,
            resume_last: false,
            list_conversations: None,
            show_conversation: None,
            model: "test-model".to_string(),
            verbose_help: false,
        };

        assert_eq!(config.image_sources.len(), 0);

        config.add_clipboard_image();

        assert_eq!(config.image_sources.len(), 1);
        match &config.image_sources[0] {
            ImageSource::Clipboard => (),
            ImageSource::File(_) => panic!("Expected clipboard image source"),
        }
    }
}
