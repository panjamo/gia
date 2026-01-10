use crate::constants::get_default_model;
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

    pub text_files: Vec<String>,
    pub output_mode: OutputMode,
    pub resume_conversation: Option<String>, // None = new, Some("") = latest, Some(id) = specific
    pub resume_last: bool,                   // true = resume latest conversation
    pub list_conversations: Option<usize>, // None = don't list, Some(n) = list top n, Some(0) = list all
    pub show_conversation: Option<String>, // Some(id) = show specific conversation
    pub model: String,
    pub record_audio: bool,                  // true = record audio input
    pub audio_device: Option<String>,        // None = default/env, Some(name) = specific device
    pub list_audio_devices: bool,            // true = list audio devices and exit
    pub roles: Vec<String>,                  // role names to load from ~/.gia/<role>.md
    pub ordered_content: Vec<ContentSource>, // ordered content for multimodal requests
    pub spinner: bool,                       // true = show spinner during AI request
    pub no_save: bool, // true = don't save to conversation history (transcribe-only mode)
    // Tool calling options
    pub enable_tools: bool,   // true = enable tool/function calling
    pub tool_allow_cwd: bool, // true = allow tools to access current working directory
    pub tool_allowed_dir: Option<String>, // Some(dir) = allow tools to access specific directory
    pub tool_disable: Vec<String>, // list of tools to disable
    pub allow_command_execution: bool, // true = allow ExecuteCommandTool
    pub command_timeout: u64, // command execution timeout in seconds
    pub confirm_commands: bool, // true = require confirmation before executing commands
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
            audio_device: matches.get_one::<String>("audio-device").cloned(),
            list_audio_devices: matches.get_flag("list-audio-devices"),
            roles,
            ordered_content: Vec::new(), // will be populated in input.rs
            spinner: matches.get_flag("spinner"),
            no_save: matches.get_flag("no-save"),
            // Tool calling options
            enable_tools: matches.get_flag("enable-tools"),
            tool_allow_cwd: matches.get_flag("tool-allow-cwd"),
            tool_allowed_dir: matches.get_one::<String>("tool-allowed-dir").cloned(),
            tool_disable: matches
                .get_many::<String>("tool-disable")
                .unwrap_or_default()
                .cloned()
                .collect(),
            allow_command_execution: matches.get_flag("allow-command-execution"),
            command_timeout: matches
                .get_one::<String>("command-timeout")
                .and_then(|s| s.parse().ok())
                .unwrap_or(30),
            confirm_commands: matches.get_flag("confirm-commands"),
        }
    }

    fn build_cli() -> Command {
        Command::new("gia")
            .version(env!("GIA_VERSION"))
            .about("AI CLI tool using Google Gemini API (stdout default)")
            .next_help_heading("Input Options")
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
                Arg::new("file")
                    .short('f')
                    .long("file")
                    .help("Add file content to prompt (can be used multiple times). Automatically detects media files (jpg, png, mp4, etc.) vs text files. Supports files and directories (processes all files recursively)")
                    .value_name("FILE_OR_DIR")
                    .action(clap::ArgAction::Append),
            )
            .arg(
                Arg::new("record-audio")
                    .short('a')
                    .long("record-audio")
                    .help("Record audio input natively (no external dependencies required)")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                Arg::new("audio-device")
                    .long("audio-device")
                    .help("Specify audio input device name for recording. Overrides GIA_AUDIO_DEVICE environment variable. Use --list-audio-devices to see available devices.")
                    .value_name("DEVICE")
                    .action(clap::ArgAction::Set),
            )
            .arg(
                Arg::new("list-audio-devices")
                    .long("list-audio-devices")
                    .help("List all available audio input devices and exit")
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
            .next_help_heading("Output Options")
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
            .next_help_heading("Conversation Management")
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
            .next_help_heading("Other Options")
            .arg(
                Arg::new("model")
                    .short('m')
                    .long("model")
                    .help("Specify the model to use. Format: 'provider::model' or just 'model' for Gemini (e.g., 'ollama::llama3.2', 'gemini-2.5-flash-lite'). Can be set via GIA_DEFAULT_MODEL environment variable.")
                    .value_name("MODEL")
                    .default_value(get_default_model())
                    .action(clap::ArgAction::Set),
            )
            .arg(
                Arg::new("verbose-help")
                    .long("verbose-help")
                    .help("Open browser with detailed documentation on GitHub")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                Arg::new("spinner")
                    .long("spinner")
                    .help("Show visual spinner during AI request (requires giagui)")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                Arg::new("no-save")
                    .long("no-save")
                    .help("Don't save to conversation history (transcribe-only mode)")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                Arg::new("completions")
                    .long("completions")
                    .help("Generate shell completion script")
                    .value_name("SHELL")
                    .value_parser(["bash", "zsh", "fish", "powershell", "nushell"])
                    .action(clap::ArgAction::Set),
            )
            .next_help_heading("Tool Options (Experimental)")
            .arg(
                Arg::new("enable-tools")
                    .long("enable-tools")
                    .help("Enable tool/function calling (allows AI to read files, list directories, etc.)")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                Arg::new("tool-allow-cwd")
                    .long("tool-allow-cwd")
                    .help("Allow tools to access current working directory (requires --enable-tools)")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                Arg::new("tool-allowed-dir")
                    .long("tool-allowed-dir")
                    .help("Allow tools to access specific directory (requires --enable-tools)")
                    .value_name("DIR")
                    .action(clap::ArgAction::Set),
            )
            .arg(
                Arg::new("tool-disable")
                    .long("tool-disable")
                    .help("Disable specific tools (comma-separated: read_file,write_file,list_directory,search_web,execute_command)")
                    .value_name("TOOLS")
                    .value_delimiter(',')
                    .action(clap::ArgAction::Append),
            )
            .arg(
                Arg::new("allow-command-execution")
                    .long("allow-command-execution")
                    .help("Allow ExecuteCommandTool to run shell commands (requires --enable-tools). DANGEROUS: Use with caution!")
                    .action(clap::ArgAction::SetTrue),
            )
            .arg(
                Arg::new("command-timeout")
                    .long("command-timeout")
                    .help("Command execution timeout in seconds (default: 30)")
                    .value_name("SECS")
                    .default_value("30")
                    .action(clap::ArgAction::Set),
            )
            .arg(
                Arg::new("confirm-commands")
                    .long("confirm-commands")
                    .help("Require user confirmation before executing each command (recommended with --allow-command-execution)")
                    .action(clap::ArgAction::SetTrue),
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
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    #[test]
    #[serial]
    fn test_default_model_from_env_var() {
        // Clean up any existing environment variable first
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };

        // Test without environment variable - should use hardcoded default
        let config = Config::from_args_with_test(&[]);
        assert_eq!(config.model, "gemini-2.5-flash-lite");

        // Test with environment variable
        unsafe { env::set_var("GIA_DEFAULT_MODEL", "gemini-2.5-pro") };
        let config = Config::from_args_with_test(&[]);
        assert_eq!(config.model, "gemini-2.5-pro");

        // Test with Ollama format
        unsafe { env::set_var("GIA_DEFAULT_MODEL", "ollama::llama3.2") };
        let config = Config::from_args_with_test(&[]);
        assert_eq!(config.model, "ollama::llama3.2");

        // Test that explicit command line argument overrides environment variable
        unsafe { env::set_var("GIA_DEFAULT_MODEL", "gemini-2.5-pro") };
        let config = Config::from_args_with_test(&["--model", "gemini-2.0-flash"]);
        assert_eq!(config.model, "gemini-2.0-flash");

        // Clean up
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };
    }

    impl Config {
        fn from_args_with_test(args: &[&str]) -> Self {
            let matches = Self::build_cli()
                .try_get_matches_from(std::iter::once("gia").chain(args.iter().copied()))
                .unwrap();

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
                audio_device: matches.get_one::<String>("audio-device").cloned(),
                list_audio_devices: matches.get_flag("list-audio-devices"),
                roles,
                ordered_content: Vec::new(),
                spinner: matches.get_flag("spinner"),
                no_save: matches.get_flag("no-save"),
                // Tool calling options
                enable_tools: matches.get_flag("enable-tools"),
                tool_allow_cwd: matches.get_flag("tool-allow-cwd"),
                tool_allowed_dir: matches.get_one::<String>("tool-allowed-dir").cloned(),
                tool_disable: matches
                    .get_many::<String>("tool-disable")
                    .unwrap_or_default()
                    .cloned()
                    .collect(),
                allow_command_execution: matches.get_flag("allow-command-execution"),
                command_timeout: matches
                    .get_one::<String>("command-timeout")
                    .and_then(|s| s.parse().ok())
                    .unwrap_or(30),
                confirm_commands: matches.get_flag("confirm-commands"),
            }
        }
    }
}
