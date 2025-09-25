use clap::{Arg, Command};

#[derive(Debug)]
pub enum OutputMode {
    Stdout,
    Clipboard,
    ClipboardWithPreview,
}

#[derive(Debug)]
pub struct Config {
    pub prompt: String,
    pub use_clipboard_input: bool,
    pub use_stdin_input: bool,
    pub output_mode: OutputMode,
    pub resume_conversation: Option<String>, // None = new, Some("") = latest, Some(id) = specific
    pub resume_last: bool,                   // true = resume latest conversation
    pub list_conversations: bool,
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
                Arg::new("clipboard-output")
                    .short('o')
                    .long("clipboard-output")
                    .help("Write output to clipboard instead of stdout")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                Arg::new("clipboard-output-with-preview")
                    .short('O')
                    .help("Write output to clipboard AND open browser preview")
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
                    .help("List all saved conversations")
                    .action(clap::ArgAction::SetTrue),
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

        let output_mode = if matches.get_flag("clipboard-output-with-preview") {
            OutputMode::ClipboardWithPreview
        } else if matches.get_flag("clipboard-output") {
            OutputMode::Clipboard
        } else {
            OutputMode::Stdout
        };

        Self {
            prompt: prompt_parts.join(" "),
            use_clipboard_input: matches.get_flag("clipboard-input"),
            use_stdin_input: matches.get_flag("stdin"),
            output_mode,
            resume_conversation,
            resume_last: matches.get_flag("resume-last"),
            list_conversations: matches.get_flag("list-conversations"),
            model: matches.get_one::<String>("model").unwrap().clone(),
        }
    }
}
