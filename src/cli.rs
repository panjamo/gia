use clap::{Arg, Command};

#[derive(Debug)]
pub enum OutputMode {
    Stdout,
    Clipboard,
    TempFileWithPreview,
}

#[derive(Debug)]
pub struct Config {
    pub prompt: String,
    pub use_clipboard_input: bool,
    pub use_stdin_input: bool,
    pub image_paths: Vec<String>,
    pub output_mode: OutputMode,
    pub resume_conversation: Option<String>, // None = new, Some("") = latest, Some(id) = specific
    pub resume_last: bool,                   // true = resume latest conversation
    pub list_conversations: Option<usize>, // None = don't list, Some(n) = list top n, Some(0) = list all
    pub show_conversation: Option<String>, // Some(id) = show specific conversation
    pub model: String,
}

impl Config {
    pub fn from_args() -> Self {
        let matches = Command::new("gia")
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
                Arg::new("stdin")
                    .short('s')
                    .long("stdin")
                    .help("Add stdin content to prompt")
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
                Arg::new("clipboard-output")
                    .short('o')
                    .long("clipboard-output")
                    .help("Write output to clipboard instead of stdout")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                Arg::new("temp-file-with-preview")
                    .short('O')
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
                    .short('v')
                    .long("show-conversation")
                    .help("Show a specific conversation in chat mode as HTML and Markdown files (or latest if no ID provided)")
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
            .get_matches();

        let prompt_parts: Vec<String> = matches
            .get_many::<String>("prompt")
            .unwrap_or_default()
            .cloned()
            .collect();

        let resume_conversation = matches.get_one::<String>("resume").cloned();

        let output_mode = if matches.get_flag("temp-file-with-preview") {
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

        Self {
            prompt: prompt_parts.join(" "),
            use_clipboard_input: matches.get_flag("clipboard-input"),
            use_stdin_input: matches.get_flag("stdin"),
            image_paths,
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
}
