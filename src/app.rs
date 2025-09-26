use anyhow::{Context, Result};
use chrono::prelude::*;
use std::fs;

use crate::browser_preview::open_markdown_preview;
use crate::cli::Config;
use crate::constants::get_context_window_limit;
use crate::conversation::{Conversation, ConversationManager, MessageRole};
use crate::input::get_input_text;
use crate::logging::{log_error, log_info};
use crate::output::output_text;
use crate::provider::{ProviderConfig, ProviderFactory};

pub async fn run_app(config: Config) -> Result<()> {
    // Initialize conversation manager
    let conversation_manager =
        ConversationManager::new().context("Failed to initialize conversation manager")?;

    // Handle list conversations command
    if let Some(limit) = config.list_conversations {
        return handle_list_conversations(&conversation_manager, limit);
    }

    // Handle show conversation command
    if let Some(conversation_id) = &config.show_conversation {
        return handle_show_conversation(&conversation_manager, conversation_id);
    }

    // Determine conversation mode and adjust prompt if needed
    let (mut conversation, final_prompt) = resolve_conversation(&config, &conversation_manager)?;

    // Get input text
    let input_text =
        get_input_text(&config, Some(&final_prompt)).context("Failed to get input text")?;

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
    conversation.truncate_if_needed(get_context_window_limit());

    // Get API keys
    let api_keys = crate::api_key::get_api_keys().context("Failed to get API keys")?;

    // Initialize AI provider
    let provider_config = ProviderConfig {
        model: config.model.clone(),
        api_keys,
    };

    let mut provider = ProviderFactory::create_provider(provider_config)
        .context("Failed to initialize AI provider")?;

    // Generate content
    log_info(&format!(
        "Sending request to {} API using model: {}",
        provider.provider_name(),
        provider.model_name()
    ));
    let response = provider
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

fn resolve_conversation(
    config: &Config,
    conversation_manager: &ConversationManager,
) -> Result<(Conversation, String)> {
    if config.resume_last {
        // Resume latest conversation with -R flag
        log_info("Attempting to resume latest conversation (-R flag)");
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
        return Ok((conv, config.prompt.clone()));
    }

    match &config.resume_conversation {
        None => {
            // New conversation
            log_info("Starting new conversation");
            Ok((Conversation::new(), config.prompt.clone()))
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
            Ok((conv, config.prompt.clone()))
        }
        Some(id) => {
            // Resume specific conversation - must be exact match
            log_info(&format!("Attempting to resume conversation: {}", id));
            let conv = conversation_manager
                .load_conversation(id)
                .with_context(|| format!("Conversation with ID '{}' not found", id))?;
            log_info(&format!("Resumed conversation: {}", conv.id));
            Ok((conv, config.prompt.clone()))
        }
    }
}

fn handle_list_conversations(
    conversation_manager: &ConversationManager,
    limit: usize,
) -> Result<()> {
    match conversation_manager.list_conversations()? {
        conversations if conversations.is_empty() => {
            println!("No saved conversations found.");
        }
        conversations => {
            let limited_conversations = if limit == 0 {
                conversations
            } else {
                conversations.into_iter().take(limit).collect()
            };

            println!("Saved Conversations:");
            println!("===================");
            for summary in limited_conversations {
                println!("{}", summary.format_for_display());
            }
            println!();
            println!("Use 'gia --resume <id>' to continue a conversation.");
            println!("Use 'gia --resume' to continue the most recent conversation.");
        }
    }
    Ok(())
}

fn handle_show_conversation(
    conversation_manager: &ConversationManager,
    conversation_id: &str,
) -> Result<()> {
    let conversation = if conversation_id.is_empty() {
        // Load the latest conversation
        log_info("Loading latest conversation");
        match conversation_manager.get_latest_conversation()? {
            Some(conv) => {
                log_info(&format!("Found latest conversation: {}", conv.id));
                conv
            }
            None => {
                println!("No conversations found.");
                return Ok(());
            }
        }
    } else {
        // Load specific conversation
        log_info(&format!("Loading conversation: {}", conversation_id));
        conversation_manager
            .load_conversation(conversation_id)
            .with_context(|| format!("Conversation with ID '{}' not found", conversation_id))?
    };

    // Get outputs directory and create it if it doesn't exist
    let outputs_dir = get_outputs_dir()?;
    if !outputs_dir.exists() {
        fs::create_dir_all(&outputs_dir).context("Failed to create outputs directory")?;
        log_info(&format!("Created outputs directory: {:?}", outputs_dir));
    }

    // Generate markdown content
    let markdown_content = conversation.format_as_chat_markdown();

    // Create filename based on conversation ID and timestamp
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    let filename = format!("conversation_{}_{}.md", conversation.id, timestamp);
    let md_file_path = outputs_dir.join(filename);

    // Write markdown file
    fs::write(&md_file_path, &markdown_content)
        .context("Failed to write conversation markdown file")?;

    log_info(&format!("Created markdown file: {:?}", md_file_path));

    // Open browser preview (which will also create HTML file)
    if let Err(e) = open_markdown_preview(&markdown_content, &md_file_path) {
        log_error(&format!("Failed to open browser preview: {}", e));
    } else {
        log_info("Opened browser preview");
    }

    println!("Conversation displayed in browser and saved to:");
    println!("Markdown: {}", md_file_path.display());
    println!("HTML: {}", md_file_path.with_extension("html").display());

    Ok(())
}

fn get_outputs_dir() -> Result<std::path::PathBuf> {
    let home_dir =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
    Ok(home_dir.join(".gia").join("outputs"))
}
