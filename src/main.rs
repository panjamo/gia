mod logging;
mod gemini;
mod clipboard;

use anyhow::{Context, Result};
use clap::{Arg, Command};
use std::io::{self, Read};
use tokio;

use crate::gemini::GeminiClient;
use crate::clipboard::{read_clipboard, write_clipboard};
use crate::logging::{init_logging, log_info, log_debug, log_error};

#[derive(Debug)]
struct Config {
    prompt: Option<String>,
    use_clipboard_input: bool,
    use_clipboard_output: bool,
}

impl Config {
    fn from_args() -> Self {
        let matches = Command::new("gia")
            .version("0.1.0")
            .about("AI CLI tool using Google Gemini API")
            .arg(
                Arg::new("prompt")
                    .short('p')
                    .long("prompt")
                    .value_name("TEXT")
                    .help("Prompt text for the AI")
                    .required(false)
            )
            .arg(
                Arg::new("clipboard-input")
                    .short('i')
                    .long("clipboard-input")
                    .help("Read prompt from clipboard instead of stdin")
                    .action(clap::ArgAction::SetTrue)
            )
            .arg(
                Arg::new("clipboard-output")
                    .short('o')
                    .long("clipboard-output")
                    .help("Write response to clipboard instead of stdout")
                    .action(clap::ArgAction::SetTrue)
            )
            .get_matches();

        Self {
            prompt: matches.get_one::<String>("prompt").cloned(),
            use_clipboard_input: matches.get_flag("clipboard-input"),
            use_clipboard_output: matches.get_flag("clipboard-output"),
        }
    }
}

fn read_stdin() -> Result<String> {
    log_debug("Reading from stdin");
    let mut buffer = String::new();
    io::stdin().read_to_string(&mut buffer)
        .context("Failed to read from stdin")?;
    
    log_info(&format!("Read {} characters from stdin", buffer.len()));
    Ok(buffer)
}

fn get_input_text(config: &Config) -> Result<String> {
    let mut input_text = String::new();
    
    // Add prompt prefix if provided
    if let Some(ref prompt) = config.prompt {
        log_info("Adding command line prompt as prefix");
        input_text.push_str(prompt);
        input_text.push_str("\n\n");
    }
    
    // Get main input from stdin or clipboard
    let main_input = if config.use_clipboard_input {
        log_info("Reading input from clipboard");
        read_clipboard()?
    } else {
        log_info("Reading input from stdin");
        read_stdin()?
    };
    
    input_text.push_str(&main_input);
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
    let input_text = get_input_text(&config)
        .context("Failed to get input text")?;

    if input_text.trim().is_empty() {
        log_error("No input text provided");
        eprintln!("Error: No input text provided. Use --prompt or provide input via stdin/clipboard.");
        std::process::exit(1);
    }

    log_info(&format!("Processing prompt with {} characters", input_text.len()));

    // Initialize Gemini client
    let client = GeminiClient::new()
        .context("Failed to initialize Gemini client")?;

    // Generate content
    log_info("Sending request to Gemini API");
    let response = client.generate_content(&input_text)
        .await
        .context("Failed to generate content")?;

    // Output response
    output_text(&response, &config)
        .context("Failed to output response")?;

    log_info("Successfully completed request");
    Ok(())
}