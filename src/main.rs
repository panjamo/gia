mod api_key;
mod clipboard;
mod gemini;
mod logging;

use anyhow::{Context, Result};
use clap::{Arg, Command};
use std::io::{self, Read};

use crate::clipboard::{read_clipboard, write_clipboard};
use crate::gemini::GeminiClient;
use crate::logging::{init_logging, log_debug, log_error, log_info};

#[derive(Debug)]
struct Config {
    prompt: String,
    use_clipboard_input: bool,
    use_stdin_input: bool,
    use_clipboard_output: bool,
}

impl Config {
    fn from_args() -> Self {
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
            .get_matches();

        let prompt_parts: Vec<String> = matches
            .get_many::<String>("prompt")
            .unwrap_or_default()
            .cloned()
            .collect();

        Self {
            prompt: prompt_parts.join(" "),
            use_clipboard_input: matches.get_flag("clipboard-input"),
            use_stdin_input: matches.get_flag("stdin"),
            use_clipboard_output: matches.get_flag("clipboard-output"),
        }
    }
}

fn read_stdin() -> Result<String> {
    log_debug("Reading from stdin");
    let mut buffer = String::new();
    io::stdin()
        .read_to_string(&mut buffer)
        .context("Failed to read from stdin")?;

    log_info(&format!("Read {} characters from stdin", buffer.len()));
    Ok(buffer)
}

fn get_input_text(config: &Config) -> Result<String> {
    let mut input_text = String::new();

    // Start with command line prompt
    if !config.prompt.is_empty() {
        log_info("Using command line prompt");
        input_text.push_str(&config.prompt);
    }

    // Add additional input if requested
    if config.use_clipboard_input {
        log_info("Adding clipboard input");
        let clipboard_input = read_clipboard()?;
        if !input_text.is_empty() {
            input_text.push_str("\n\n");
        }
        input_text.push_str(&clipboard_input);
    }

    if config.use_stdin_input {
        log_info("Adding stdin input");
        let stdin_input = read_stdin()?;
        if !input_text.is_empty() {
            input_text.push_str("\n\n");
        }
        input_text.push_str(&stdin_input);
    }

    Ok(input_text)
}

fn output_text(text: &str, config: &Config) -> Result<()> {
    if config.use_clipboard_output {
        log_info("Writing response to clipboard");
        write_clipboard(text)
    } else {
        log_info("Writing response to stdout");
        print!("{}", text);
        Ok(())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    init_logging();

    log_info("Starting gia - AI CLI tool");

    let config = Config::from_args();
    log_debug(&format!("Configuration: {:?}", config));

    // Get input text
    let input_text = get_input_text(&config).context("Failed to get input text")?;

    if input_text.trim().is_empty() {
        log_error("No input text provided");
        eprintln!("Error: No input text provided. Provide prompt as command line arguments or use -c/-s for additional input.");
        std::process::exit(1);
    }

    log_info(&format!(
        "Processing prompt with {} characters",
        input_text.len()
    ));

    // Initialize Gemini client
    let client = GeminiClient::new().context("Failed to initialize Gemini client")?;

    // Generate content
    log_info("Sending request to Gemini API");
    let response = client
        .generate_content(&input_text)
        .await
        .context("Failed to generate content")?;

    // Output response
    output_text(&response, &config).context("Failed to output response")?;

    log_info("Successfully completed request");
    Ok(())
}
