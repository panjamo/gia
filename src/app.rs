use anyhow::{Context, Result};

use crate::cli::Config;
use crate::constants::DEFAULT_CONTEXT_WINDOW_LIMIT;
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
    if config.list_conversations {
        return handle_list_conversations(&conversation_manager);
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
    conversation.truncate_if_needed(DEFAULT_CONTEXT_WINDOW_LIMIT);

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
