use anyhow::{Context, Result};

use crate::cli::{Config, ContentSource};
use crate::constants::get_context_window_limit;
use crate::content_part_wrapper::{ChatMessageWrapper, ContentPartWrapper, MessageContentWrapper};
use crate::conversation::TokenUsage;
use crate::conversation::{Conversation, ConversationManager, ResourceInfo, ResourceType};
use crate::input::get_input_text;
use crate::logging::{log_error, log_info, setup_conversation_file_logging};
use crate::output::output_text_with_usage;
use crate::provider::{ProviderConfig, ProviderFactory};
use crate::spinner::SpinnerProcess;

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

    // Determine conversation mode and adjust prompt if needed
    let (mut conversation, final_prompt) =
        resolve_conversation(&config, &conversation_manager, &config.model)?;

    // Setup file logging for this conversation if GIA_LOG_TO_FILE is set
    setup_conversation_file_logging(&conversation.id)
        .context("Failed to setup conversation file logging")?;

    // Get input content (this may modify config to add clipboard images)
    get_input_text(&mut config, Some(&final_prompt)).context("Failed to get input text")?;

    if config.ordered_content.is_empty() {
        log_error("No input content provided");
        eprintln!(
            "Error: No input content provided. Provide prompt as command line arguments or use -c/-f/-i for additional input."
        );
        std::process::exit(1);
    }

    log_info(&format!(
        "Processing prompt with {} content source(s)",
        config.ordered_content.len()
    ));

    // Truncate conversation if it's getting too long
    conversation.truncate_if_needed(get_context_window_limit());

    // Get API keys - only required for non-Ollama providers
    let api_keys = if config.model.to_lowercase().starts_with("ollama::") {
        Vec::new()
    } else {
        crate::api_key::get_api_keys().context("Failed to get API keys")?
    };

    // Initialize AI provider
    let provider_config = ProviderConfig {
        model: config.model.clone(),
        api_keys,
    };

    let mut provider = ProviderFactory::create_provider(provider_config)
        .context("Failed to initialize AI provider")?;

    // 1. Build new user message wrapper from ordered content
    let content_part_wrappers = build_content_part_wrappers(&config.ordered_content)?;

    log_info(&format!(
        "Created user message with {} content part(s)",
        content_part_wrappers.len()
    ));

    let new_user_message_wrapper = ChatMessageWrapper {
        role: "User".to_string(),
        content: MessageContentWrapper::Parts {
            parts: content_part_wrappers,
        },
    };

    // 2. Convert conversation history + new message to genai ChatMessages for API
    let mut all_genai_messages = conversation.to_genai_messages()?;
    let history_message_count = all_genai_messages.len();
    all_genai_messages.push(new_user_message_wrapper.to_genai_chat_message()?);

    log_info(&format!(
        "Total messages for API: {} ({} from history + 1 new)",
        all_genai_messages.len(),
        history_message_count
    ));

    // 3. Generate content using chat messages
    log_info(&format!(
        "Sending chat request to {} API using model: {}",
        provider.provider_name(),
        provider.model_name()
    ));

    // Start spinner if requested
    let _spinner = if config.spinner {
        Some(SpinnerProcess::start())
    } else {
        None
    };

    let ai_response = provider
        .generate_content_with_chat_messages(all_genai_messages)
        .await
        .context("Failed to generate content")?;

    // Spinner is automatically killed when dropped here
    drop(_spinner);

    let response = ai_response.content;
    let usage = ai_response.usage;

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
            _ => None, // Skip CommandLinePrompt
        };

        if let Some(res) = resource {
            resources.push(res);
        }
    }

    // 4. Create assistant message wrapper
    let assistant_message_wrapper = ChatMessageWrapper {
        role: "Assistant".to_string(),
        content: MessageContentWrapper::Text {
            text: response.clone(),
        },
    };

    // 5. Add messages to conversation with token usage
    conversation.add_message_with_usage(new_user_message_wrapper, resources, TokenUsage::default());
    conversation.add_message_with_usage(assistant_message_wrapper, Vec::new(), usage);

    // Save conversation
    conversation_manager
        .save_conversation(&conversation)
        .context("Failed to save conversation")?;

    // Save markdown
    conversation_manager
        .save_markdown(&conversation)
        .context("Failed to save markdown")?;

    // Output response
    output_text_with_usage(&response, &config, Some(usage), &conversation.id)
        .context("Failed to output response")?;

    log_info("Successfully completed request");
    Ok(())
}

fn build_content_part_wrappers(
    ordered_content: &[ContentSource],
) -> Result<Vec<ContentPartWrapper>> {
    log_info(&format!(
        "=== Building multimodal content from {} source(s) ===",
        ordered_content.len()
    ));

    let mut wrappers = Vec::new();

    for (index, content_source) in ordered_content.iter().enumerate() {
        match content_source {
            ContentSource::CommandLinePrompt(prompt) => {
                log_info(&format!(
                    "[{}] Prompt: {} characters",
                    index + 1,
                    prompt.len()
                ));
                wrappers.push(ContentPartWrapper::Prompt(prompt.clone()));
            }
            ContentSource::RoleDefinition(name, content, is_task) => {
                let item_type = if *is_task { "Task" } else { "Role" };
                log_info(&format!(
                    "[{}] {}: '{}' ({} characters)",
                    index + 1,
                    item_type,
                    name,
                    content.len()
                ));
                wrappers.push(ContentPartWrapper::RoleDefinition {
                    name: name.clone(),
                    content: content.clone(),
                    is_task: *is_task,
                });
            }
            ContentSource::TextFile(path, content) => {
                log_info(&format!(
                    "[{}] Text file: {} ({} characters)",
                    index + 1,
                    path,
                    content.len()
                ));
                wrappers.push(ContentPartWrapper::TextFile {
                    path: path.clone(),
                    content: content.clone(),
                });
            }
            ContentSource::ClipboardText(text) => {
                log_info(&format!(
                    "[{}] Clipboard text: {} characters",
                    index + 1,
                    text.len()
                ));
                wrappers.push(ContentPartWrapper::ClipboardText(text.clone()));
            }
            ContentSource::StdinText(text) => {
                log_info(&format!(
                    "[{}] Stdin text: {} characters",
                    index + 1,
                    text.len()
                ));
                wrappers.push(ContentPartWrapper::StdinText(text.clone()));
            }
            ContentSource::ImageFile(path) => {
                let mime_type = crate::image::get_mime_type(std::path::Path::new(path))?;
                let data = crate::image::read_media_as_base64(path)?;
                log_info(&format!(
                    "[{}] Image file: {} (type: {}, {} base64 chars)",
                    index + 1,
                    path,
                    mime_type,
                    data.len()
                ));
                wrappers.push(ContentPartWrapper::Image {
                    path: Some(path.clone()),
                    mime_type,
                    data,
                });
            }
            ContentSource::ClipboardImage => {
                let image_data = crate::clipboard::read_clipboard_image()?;
                let data = crate::clipboard::convert_image_data_to_base64(&image_data)?;
                let mime_type = "image/png".to_string(); // Clipboard images are typically PNG
                log_info(&format!(
                    "[{}] Clipboard image (type: {}, {} base64 chars)",
                    index + 1,
                    mime_type,
                    data.len()
                ));
                wrappers.push(ContentPartWrapper::Image {
                    path: None,
                    mime_type,
                    data,
                });
            }
            ContentSource::AudioRecording(path) => {
                // Audio files use the same image MIME type detection for now
                let mime_type = crate::image::get_mime_type(std::path::Path::new(path))?;
                let data = crate::image::read_media_as_base64(path)?;
                log_info(&format!(
                    "[{}] Audio recording: {} (type: {}, {} base64 chars)",
                    index + 1,
                    path,
                    mime_type,
                    data.len()
                ));
                wrappers.push(ContentPartWrapper::Audio {
                    path: path.clone(),
                    mime_type,
                    data,
                });
            }
        }
    }

    log_info(&format!(
        "=== Built {} content part(s) for API request ===",
        wrappers.len()
    ));

    Ok(wrappers)
}

fn resolve_conversation(
    config: &Config,
    conversation_manager: &ConversationManager,
    model: &str,
) -> Result<(Conversation, String)> {
    if config.resume_last {
        // Resume latest conversation with -R flag
        log_info("Attempting to resume latest conversation (-R flag)");
        let conv = if let Some(conv) = conversation_manager.get_latest_conversation()? {
            log_info(&format!("Resumed conversation: {}", conv.id));
            conv
        } else {
            log_info("No previous conversations found, starting new conversation");
            Conversation::new_with_prompt(model.to_string(), &config.prompt)
        };
        return Ok((conv, config.prompt.clone()));
    }

    match &config.resume_conversation {
        None => {
            // New conversation
            log_info("Starting new conversation");
            Ok((
                Conversation::new_with_prompt(model.to_string(), &config.prompt),
                config.prompt.clone(),
            ))
        }
        Some(id) if id.is_empty() => {
            // Resume latest conversation
            log_info("Attempting to resume latest conversation");
            let conv = if let Some(conv) = conversation_manager.get_latest_conversation()? {
                log_info(&format!("Resumed conversation: {}", conv.id));
                conv
            } else {
                log_info("No previous conversations found, starting new conversation");
                Conversation::new_with_prompt(model.to_string(), &config.prompt)
            };
            Ok((conv, config.prompt.clone()))
        }
        Some(id) => {
            // Resume specific conversation - can be index, hash, or full ID
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
    use std::io::Write;
    use tabwriter::TabWriter;

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

            let mut tw = TabWriter::new(std::io::stdout());

            // Write header
            writeln!(tw, "index\tmessages\tage\tid\tpreview").context("Failed to write header")?;

            // Write data rows
            for (index, summary) in limited_conversations.iter().enumerate() {
                let (preview, id, age, messages) = summary.format_as_table_columns();
                writeln!(tw, "{}\t{}\t{}\t{}\t{}", index, messages, age, id, preview)
                    .context("Failed to write row")?;
            }

            tw.flush().context("Failed to flush table output")?;
        }
    }
    Ok(())
}

fn handle_show_conversation(
    conversation_manager: &ConversationManager,
    conversation_id: &str,
    config: &Config,
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

    // If TTS is enabled, extract and speak the conversation
    if let crate::cli::OutputMode::Tts(lang) = &config.output_mode {
        use crate::output::speak_conversation;
        speak_conversation(&conversation, lang)?;
    }

    Ok(())
}
