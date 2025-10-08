use clap::{Arg, Command};
use clap_complete::{generate, shells};
use clap_complete_nushell::Nushell;

#[derive(Debug, Clone)]
pub enum OutputMode {
    Stdout,
    Clipboard,
    TempFileWithPreview,
    Tts(String), // language code (e.g., "de-DE", "en-US")
}

#[derive(Debug, Clone)]
pub enum ImageSource {
    File(String),
}

#[derive(Debug, Clone)]
pub enum ContentSource {
    CommandLinePrompt(String),
    AudioRecording(String), // file path
    ClipboardText(String),
    StdinText(String),
    TextFile(String, String), // (file_path, content)
    ImageFile(String),        // file path
    ClipboardImage,
    RoleDefinition(String, String, bool), // (name, content, is_task)
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
    pub record_audio: bool,                  // true = record audio input
    pub roles: Vec<String>,                  // role names to load from ~/.gia/<role>.md
    pub ordered_content: Vec<ContentSource>, // ordered content for multimodal requests
}

impl Config {
    pub fn from_args() -> Self {
        let matches = Self::build_cli().get_matches();

        // Handle completions generation immediately
        if let Some(shell) = matches.get_one::<String>("completions") {
            Self::handle_completions(shell);
            std::process::exit(0);
        }

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
        } else if let Some(lang) = matches.get_one::<String>("tts-output").cloned() {
            OutputMode::Tts(lang)
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

        let roles: Vec<String> = matches
            .get_many::<String>("role")
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
            record_audio: matches.get_flag("record-audio"),
            roles,
            ordered_content: Vec::new(), // will be populated in input.rs
        }
    }

    fn build_cli() -> Command {
        Command::new("gia")
            .version(env!("GIA_VERSION"))
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
                    .help("Add text file content to prompt (can be used multiple times). Supports files and directories (processes all files recursively)")
                    .value_name("FILE_OR_DIR")
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
                Arg::new("tts-output")
                    .short('T')
                    .long("tts")
                    .help("Use text-to-speech for output with optional language (e.g., 'de-DE', 'en-US'). Default: de-DE")
                    .value_name("LANG")
                    .num_args(0..=1)
                    .default_missing_value("de-DE")
                    .action(clap::ArgAction::Set),
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
            .arg(
                Arg::new("record-audio")
                    .short('a')
                    .long("record-audio")
                    .help("Record audio input using ffmpeg (requires ffmpeg to be installed)")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                Arg::new("role")
                    .short('t')
                    .long("role")
                    .help("Load role/task from ~/.gia/roles/<name>.md or ~/.gia/tasks/<name>.md (can be used multiple times)")
                    .value_name("NAME")
                    .action(clap::ArgAction::Append),
            )
            .arg(
                Arg::new("completions")
                    .long("completions")
                    .help("Generate shell completion script")
                    .value_name("SHELL")
                    .value_parser(["bash", "zsh", "fish", "powershell", "nushell"])
                    .action(clap::ArgAction::Set),
            )
    }

    fn handle_completions(shell: &str) {
        let mut cmd = Self::build_cli();
        let bin_name = "gia";

        match shell {
            "bash" => generate(shells::Bash, &mut cmd, bin_name, &mut std::io::stdout()),
            "zsh" => generate(shells::Zsh, &mut cmd, bin_name, &mut std::io::stdout()),
            "fish" => generate(shells::Fish, &mut cmd, bin_name, &mut std::io::stdout()),
            "powershell" => generate(
                shells::PowerShell,
                &mut cmd,
                bin_name,
                &mut std::io::stdout(),
            ),
            "nushell" => generate(Nushell, &mut cmd, bin_name, &mut std::io::stdout()),
            _ => eprintln!("Unsupported shell: {}", shell),
        }
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
mod tests {}
