use anyhow::{Context, Result};
use std::io::Write;
use tabwriter::TabWriter;

use crate::cli::{Config, ContentSource};
use crate::constants::get_context_window_limit;
use crate::conversation::{
    Conversation, ConversationManager, MessageRole, ResourceInfo, ResourceType,
};
use crate::input::{get_input_text, validate_image_sources};
use crate::logging::{log_error, log_info};
use crate::output::output_text;
use crate::provider::{ProviderConfig, ProviderFactory};

pub async fn run_app(mut config: Config) -> Result<()> {
    // Initialize conversation manager
    let conversation_manager =
        ConversationManager::new().context("Failed to initialize conversation manager")?;

    // Handle list conversations command
    if let Some(limit) = config.list_conversations {
        return handle_list_conversations(&conversation_manager, limit);
    }

    // Handle show conversation command
    if let Some(conversation_id) = &config.show_conversation {
        return handle_show_conversation(&conversation_manager, conversation_id, &config);
    }

    // Validate image sources if any are provided
    validate_image_sources(&config).context("Failed to validate image sources")?;

    // Determine conversation mode and adjust prompt if needed
    let (mut conversation, final_prompt) = resolve_conversation(&config, &conversation_manager)?;

    // Get input text (this may modify config to add clipboard images)
    let input_text =
        get_input_text(&mut config, Some(&final_prompt)).context("Failed to get input text")?;

    if input_text.trim().is_empty() {
        log_error("No input text provided");
        eprintln!("Error: No input text provided. Provide prompt as command line arguments or use -c/-f/-i for additional input.");
        std::process::exit(1);
    }

    log_info(&format!(
        "Processing prompt with {} characters",
        input_text.len()
    ));

    // Truncate conversation if it's getting too long
    conversation.truncate_if_needed(get_context_window_limit());

    // Add conversation history to ordered content if we have any
    if !conversation.messages.is_empty() {
        // Build conversation history text
        let mut history = String::new();
        for message in &conversation.messages {
            match message.role {
                MessageRole::User => history.push_str("User: "),
                MessageRole::Assistant => history.push_str("Assistant: "),
            }
            history.push_str(&message.content);
            history.push('\n');
        }

        // Insert conversation history at the beginning of ordered content
        config
            .ordered_content
            .insert(0, crate::cli::ContentSource::ConversationHistory(history));
    }

    // Get API keys
    let api_keys = crate::api_key::get_api_keys().context("Failed to get API keys")?;

    // Initialize AI provider
    let provider_config = ProviderConfig {
        model: config.model.clone(),
        api_keys,
    };

    let mut provider = ProviderFactory::create_provider(provider_config)
        .context("Failed to initialize AI provider")?;

    // Generate content using ordered content sources
    log_info(&format!(
        "Sending multimodal request to {} API using model: {} with {} ordered content source(s)",
        provider.provider_name(),
        provider.model_name(),
        config.ordered_content.len()
    ));

    let response = provider
        .generate_content_with_ordered_sources(&config.ordered_content)
        .await
        .context("Failed to generate content")?;

    // Build resources from ordered content
    let mut resources = Vec::new();
    for content_source in &config.ordered_content {
        let resource = match content_source {
            ContentSource::ImageFile(path) => Some(ResourceInfo {
                resource_type: ResourceType::Image,
                path: Some(path.clone()),
            }),
            ContentSource::AudioRecording(path) => Some(ResourceInfo {
                resource_type: ResourceType::Audio,
                path: Some(path.clone()),
            }),
            ContentSource::TextFile(path, _) => Some(ResourceInfo {
                resource_type: ResourceType::TextFile,
                path: Some(path.clone()),
            }),
            ContentSource::ClipboardText(_) => Some(ResourceInfo {
                resource_type: ResourceType::ClipboardText,
                path: None,
            }),
            ContentSource::ClipboardImage => Some(ResourceInfo {
                resource_type: ResourceType::ClipboardImage,
                path: None,
            }),
            ContentSource::StdinText(_) => Some(ResourceInfo {
                resource_type: ResourceType::Stdin,
                path: None,
            }),
            ContentSource::RoleDefinition(name, _, is_task) => Some(ResourceInfo {
                resource_type: if *is_task {
                    ResourceType::Task
                } else {
                    ResourceType::Role
                },
                path: Some(name.clone()),
            }),
            _ => None, // Skip CommandLinePrompt and ConversationHistory
        };

        if let Some(res) = resource {
            resources.push(res);
        }
    }

    // Add messages to conversation
    conversation.add_message(MessageRole::User, input_text, resources);
    conversation.add_message(MessageRole::Assistant, response.clone(), Vec::new());

    // Save conversation
    conversation_manager
        .save_conversation(&conversation)
        .context("Failed to save conversation")?;

    // Save markdown
    conversation_manager
        .save_markdown(&conversation)
        .context("Failed to save markdown")?;

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
        let conv = if let Some(conv) = conversation_manager.get_latest_conversation()? {
            log_info(&format!("Resumed conversation: {}", conv.id));
            conv
        } else {
            log_info("No previous conversations found, starting new conversation");
            Conversation::new()
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
            let conv = if let Some(conv) = conversation_manager.get_latest_conversation()? {
                log_info(&format!("Resumed conversation: {}", conv.id));
                conv
            } else {
                log_info("No previous conversations found, starting new conversation");
                Conversation::new()
            };
            Ok((conv, config.prompt.clone()))
        }
        Some(id) => {
            // Resume specific conversation - must be exact match
            log_info(&format!("Attempting to resume conversation: {id}"));
            let conv = conversation_manager
                .load_conversation(id)
                .with_context(|| format!("Conversation with ID '{id}' not found"))?;
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

            // Use TabWriter for aligned table output
            let mut tw = TabWriter::new(std::io::stdout());
            writeln!(&mut tw, "ID\tMSGS\tUPDATED\tAGE\tPREVIEW")
                .context("Failed to write table header")?;

            for summary in limited_conversations {
                writeln!(&mut tw, "{}", summary.format_as_table_row())
                    .context("Failed to write table row")?;
            }

            tw.flush().context("Failed to flush table output")?;
        }
    }
    Ok(())
}

fn handle_show_conversation(
    conversation_manager: &ConversationManager,
    conversation_id: &str,
    _config: &Config,
) -> Result<()> {
    let conversation = if conversation_id.is_empty() {
        // Load the latest conversation
        log_info("Loading latest conversation");
        if let Some(conv) = conversation_manager.get_latest_conversation()? {
            log_info(&format!("Found latest conversation: {}", conv.id));
            conv
        } else {
            println!("No conversations found.");
            return Ok(());
        }
    } else {
        // Load specific conversation
        log_info(&format!("Loading conversation: {conversation_id}"));
        conversation_manager
            .load_conversation(conversation_id)
            .with_context(|| format!("Conversation with ID '{conversation_id}' not found"))?
    };

    // Get the path to the existing markdown file in conversations folder
    let markdown_path = conversation_manager
        .get_markdown_path(&conversation)
        .context("Failed to get markdown path")?;

    // Read the existing markdown content
    let markdown_content =
        std::fs::read_to_string(&markdown_path).context("Failed to read existing markdown file")?;

    // Copy the markdown path to clipboard
    use crate::clipboard::write_clipboard;
    let path_str = markdown_path.to_string_lossy();
    write_clipboard(&path_str).context("Failed to copy path to clipboard")?;
    log_info(&format!("Copied markdown path to clipboard: {}", path_str));

    // Open browser preview directly using the existing markdown file
    use crate::browser_preview::open_markdown_preview;
    if let Err(e) = open_markdown_preview(&markdown_content, &markdown_path, None) {
        log_error(&format!("Failed to open browser preview: {e}"));
    } else {
        log_info("Opened browser preview");
    }

    Ok(())
}
