mod api_key;
mod clipboard;
mod conversation;
mod gemini;
mod logging;

use anyhow::{Context, Result};
use clap::{Arg, Command};
use std::io::{self, Read};

use crate::clipboard::{read_clipboard, write_clipboard};
use crate::conversation::{Conversation, ConversationManager, MessageRole};
use crate::gemini::GeminiClient;
use crate::logging::{init_logging, log_debug, log_error, log_info};

#[derive(Debug)]
struct Config {
    prompt: String,
    use_clipboard_input: bool,
    use_stdin_input: bool,
    use_clipboard_output: bool,
    resume_conversation: Option<String>, // None = new, Some("") = latest, Some(id) = specific
    list_conversations: bool,
    model: String,
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
                    .help("Specify the Gemini model to use (see https://ai.google.dev/gemini-api/docs)")
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

        Self {
            prompt: prompt_parts.join(" "),
            use_clipboard_input: matches.get_flag("clipboard-input"),
            use_stdin_input: matches.get_flag("stdin"),
            use_clipboard_output: matches.get_flag("clipboard-output"),
            resume_conversation,
            list_conversations: matches.get_flag("list-conversations"),
            model: matches.get_one::<String>("model").unwrap().clone(),
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

fn get_input_text(config: &Config, prompt_override: Option<&str>) -> Result<String> {
    let mut input_text = String::new();

    // Start with command line prompt (or override)
    let prompt_to_use = prompt_override.unwrap_or(&config.prompt);
    if !prompt_to_use.is_empty() {
        log_info("Using command line prompt");
        input_text.push_str(prompt_to_use);
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

    // Initialize conversation manager
    let conversation_manager =
        ConversationManager::new().context("Failed to initialize conversation manager")?;

    // Handle list conversations command
    if config.list_conversations {
        return handle_list_conversations(&conversation_manager);
    }

    // Determine conversation mode and adjust prompt if needed
    let (mut conversation, final_prompt) = match &config.resume_conversation {
        None => {
            // New conversation
            log_info("Starting new conversation");
            (Conversation::new(), config.prompt.clone())
        }
        Some(id) if id.is_empty() => {
            // Resume latest conversation
            log_info("Attempting to resume latest conversation");
            let conv = match conversation_manager.get_latest_conversation()? {
                Some(conv) => {
                    log_info(&format!("Resumed conversation: {}", conv.id));
                    conv
                }
                None => {
                    log_info("No previous conversations found, starting new conversation");
                    Conversation::new()
                }
            };
            (conv, config.prompt.clone())
        }
        Some(id) => {
            // Try to resume specific conversation, if not found treat id as prompt
            log_info(&format!("Attempting to resume conversation: {}", id));
            match conversation_manager.load_conversation(id) {
                Ok(conv) => {
                    log_info(&format!("Resumed conversation: {}", conv.id));
                    (conv, config.prompt.clone())
                }
                Err(_) => {
                    log_info(&format!("Conversation '{}' not found, treating as prompt and resuming latest conversation", id));
                    let conv = match conversation_manager.get_latest_conversation()? {
                        Some(conv) => {
                            log_info(&format!("Resumed latest conversation: {}", conv.id));
                            conv
                        }
                        None => {
                            log_info("No previous conversations found, starting new conversation");
                            Conversation::new()
                        }
                    };
                    // Combine the "failed ID" with the regular prompt
                    let combined_prompt = if config.prompt.is_empty() {
                        id.clone()
                    } else {
                        format!("{} {}", id, config.prompt)
                    };
                    (conv, combined_prompt)
                }
            }
        }
    };

    // Get input text
    let input_text = get_input_text(&config, Some(&final_prompt)).context("Failed to get input text")?;

    if input_text.trim().is_empty() {
        log_error("No input text provided");
        eprintln!("Error: No input text provided. Provide prompt as command line arguments or use -c/-s for additional input.");
        std::process::exit(1);
    }

    log_info(&format!(
        "Processing prompt with {} characters",
        input_text.len()
    ));

    // Build context prompt with conversation history
    let context_prompt = conversation.build_context_prompt(&input_text);

    // Truncate conversation if it's getting too long
    conversation.truncate_if_needed(8000); // Conservative limit for context window

    // Initialize Gemini client
    let mut client = GeminiClient::new(config.model.clone()).context("Failed to initialize Gemini client")?;

    // Generate content
    log_info("Sending request to Gemini API");
    let response = client
        .generate_content(&context_prompt)
        .await
        .context("Failed to generate content")?;

    // Add messages to conversation
    conversation.add_message(MessageRole::User, input_text);
    conversation.add_message(MessageRole::Assistant, response.clone());

    // Save conversation
    conversation_manager
        .save_conversation(&conversation)
        .context("Failed to save conversation")?;

    // Output response
    output_text(&response, &config).context("Failed to output response")?;

    log_info("Successfully completed request");
    Ok(())
}

fn handle_list_conversations(conversation_manager: &ConversationManager) -> Result<()> {
    match conversation_manager.list_conversations()? {
        conversations if conversations.is_empty() => {
            println!("No saved conversations found.");
        }
        conversations => {
            println!("Saved Conversations:");
            println!("===================");
            for summary in conversations {
                println!("{}", summary.format_for_display());
            }
            println!();
            println!("Use 'gia --resume <id>' to continue a conversation.");
            println!("Use 'gia --resume' to continue the most recent conversation.");
        }
    }
    Ok(())
}
