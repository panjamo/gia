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
    // Handle list audio devices command
    if config.list_audio_devices {
        return crate::audio::list_audio_devices();
    }

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

    // Get API keys - only required for non-Ollama providers
    let api_keys = if config.model.to_lowercase().starts_with("ollama::") {
        Vec::new()
    } else {
        crate::api_key::get_api_keys().context("Failed to get API keys")?
    };

    // Determine conversation mode and adjust prompt if needed
    let (mut conversation, final_prompt) =
        resolve_conversation(&config, &conversation_manager, &config.model, &api_keys)?;

    // Setup file logging for this conversation if GIA_LOG_TO_FILE is set
    setup_conversation_file_logging(&conversation.id)
        .context("Failed to setup conversation file logging")?;

    // Get input content (this may modify config to add clipboard images)
    // Note: Audio recording happens here, so spinner must start AFTER this
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

    // Start spinner now (after audio recording completes, before AI operations)
    let _spinner = if config.spinner {
        Some(SpinnerProcess::start())
    } else {
        None
    };

    // Truncate conversation if it's getting too long
    conversation.truncate_if_needed(get_context_window_limit());

    // Initialize AI provider with preferred API key index from conversation (for caching)
    let provider_config = ProviderConfig {
        model: config.model.clone(),
        api_keys: api_keys.clone(),
        preferred_api_key_index: conversation.metadata.api_key_index,
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

    // Spinner already started earlier if requested

    // Initialize tools if enabled
    let tool_executor = if config.enable_tools {
        // Show warning when tools are enabled
        eprintln!("\n⚠️  Tools enabled!");
        eprintln!("   The AI can:");
        eprintln!("   - Read and write files");
        eprintln!("   - List directories");
        eprintln!("   - Search the web");
        if config.allow_command_execution {
            eprintln!("   - Execute shell commands (DANGEROUS!)");
            if config.confirm_commands {
                eprintln!("   - You will be asked to confirm each command");
            }
        }
        eprintln!();

        Some(initialize_tool_executor(&config)?)
    } else {
        None
    };

    // Execute with or without tools
    let (response, usage, _tool_messages) = if let Some(executor) = &tool_executor {
        log_info("Tools enabled - using tool execution loop");
        execute_with_tools(&mut provider, all_genai_messages, executor, &config.model).await?
    } else {
        let ai_response = provider
            .generate_content_with_chat_messages(all_genai_messages)
            .await
            .context("Failed to generate content")?;
        (ai_response.content, ai_response.usage, Vec::new())
    };

    // Spinner is automatically killed when dropped here
    drop(_spinner);

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

    // Save conversation (only if no_save flag is not set)
    if !config.no_save {
        conversation_manager
            .save_conversation(&conversation)
            .context("Failed to save conversation")?;

        // Save markdown
        conversation_manager
            .save_markdown(&conversation)
            .context("Failed to save markdown")?;
    }

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

/// Initialize tool executor from config
///
/// KISS: Simple function that creates ToolExecutor with registered tools
/// DRY: Centralized tool registration
fn initialize_tool_executor(config: &Config) -> Result<crate::tools::ToolExecutor> {
    use crate::tools::*;
    use std::time::Duration;

    let mut registry = ToolRegistry::new();

    // Get disabled tools from config
    let disabled_tools: std::collections::HashSet<String> = config
        .tool_disable
        .iter()
        .map(|s| s.to_lowercase())
        .collect();

    // Register tools (KISS: straightforward registration)
    if !disabled_tools.contains("read_file") {
        registry.register(Box::new(ReadFileTool));
        log_info("Registered tool: read_file");
    }

    if !disabled_tools.contains("write_file") {
        registry.register(Box::new(WriteFileTool));
        log_info("Registered tool: write_file");
    }

    if !disabled_tools.contains("list_directory") {
        registry.register(Box::new(ListDirectoryTool));
        log_info("Registered tool: list_directory");
    }

    if !disabled_tools.contains("search_web") {
        registry.register(Box::new(SearchWebTool));
        log_info("Registered tool: search_web");
    }

    if config.allow_command_execution && !disabled_tools.contains("execute_command") {
        registry.register(Box::new(ExecuteCommandTool));
        log_info("Registered tool: execute_command");
    }

    // Build security context from config
    let mut security = SecurityContext::new()
        .with_max_file_size(10 * 1024 * 1024) // 10MB
        .with_allow_web_search(true)
        .with_allow_command_execution(config.allow_command_execution)
        .with_command_timeout(Duration::from_secs(config.command_timeout))
        .with_confirm_commands(config.confirm_commands);

    if config.tool_allow_cwd {
        security = security
            .allow_current_dir()
            .context("Failed to allow current directory")?;
    }

    if let Some(ref dir) = config.tool_allowed_dir {
        security = security.with_allowed_dir(dir);
    }

    log_info(&format!(
        "Tool executor initialized with {} tool(s)",
        registry.len()
    ));

    Ok(ToolExecutor::new(registry, security))
}

/// Create Google Search grounding tool for Gemini
///
/// When GIA_SEARCH_MODE is unset, this enables Gemini's built-in Google Search grounding.
/// This is a paid feature that provides automatic web search with citations.
fn create_google_search_grounding_tool() -> genai::chat::Tool {
    use serde_json::json;

    genai::chat::Tool::new("google_search_retrieval").with_config(json!({
        "google_search_retrieval": {}
    }))
}

/// Check if we should use Gemini grounding (GIA_SEARCH_MODE unset + Gemini model)
fn should_use_gemini_grounding(model: &str) -> bool {
    let search_mode = std::env::var("GIA_SEARCH_MODE").ok();
    let is_gemini = !model.contains("::") || model.starts_with("gemini");

    search_mode.is_none() && is_gemini
}

/// Execute with tools (tool calling loop)
///
/// KISS: Simple loop until no more tool calls
/// This function implements the tool execution loop:
/// 1. Send messages with tools to LLM
/// 2. If LLM returns tool calls, execute them and loop back
/// 3. If LLM returns text, return final response
async fn execute_with_tools(
    provider: &mut Box<dyn crate::provider::AiProvider>,
    mut messages: Vec<genai::chat::ChatMessage>,
    executor: &crate::tools::ToolExecutor,
    model: &str,
) -> Result<(String, TokenUsage, Vec<ChatMessageWrapper>)> {
    use genai::chat::ChatRequest;

    const MAX_TOOL_ITERATIONS: usize = 10;

    let mut conversation_wrappers: Vec<ChatMessageWrapper> = Vec::new();
    let mut total_usage = TokenUsage::default();
    let mut tools = executor.registry().to_genai_tools();

    if should_use_gemini_grounding(model) {
        tools.push(create_google_search_grounding_tool());
        log_info("Added Google Search grounding for Gemini (paid feature)");
        log_info("Set GIA_SEARCH_MODE=duckduckgo or GIA_SEARCH_MODE=brave for free alternatives");
    }

    for iteration in 0..MAX_TOOL_ITERATIONS {
        log_info(&format!(
            "Tool iteration {}/{}",
            iteration + 1,
            MAX_TOOL_ITERATIONS
        ));

        let chat_request = ChatRequest::new(messages.clone()).with_tools(tools.clone());
        let chat_response = provider
            .generate_content_with_request(chat_request)
            .await
            .context("Failed to generate content with tools")?;

        if let Some(prompt) = chat_response.usage.prompt_tokens {
            total_usage.prompt_tokens =
                Some(total_usage.prompt_tokens.unwrap_or(0) + (prompt as u32));
        }
        if let Some(completion) = chat_response.usage.completion_tokens {
            total_usage.completion_tokens =
                Some(total_usage.completion_tokens.unwrap_or(0) + (completion as u32));
        }
        if let Some(total) = chat_response.usage.total_tokens {
            total_usage.total_tokens = Some(total_usage.total_tokens.unwrap_or(0) + (total as u32));
        }

        let tool_calls = chat_response.tool_calls();

        if tool_calls.is_empty() {
            let final_text = chat_response
                .first_text()
                .unwrap_or("(no text)")
                .to_string();

            let assistant_wrapper = ChatMessageWrapper {
                role: "Assistant".to_string(),
                content: MessageContentWrapper::Text {
                    text: final_text.clone(),
                },
            };
            conversation_wrappers.push(assistant_wrapper);

            return Ok((final_text, total_usage, conversation_wrappers));
        }

        log_info(&format!("LLM requested {} tool call(s)", tool_calls.len()));

        let tool_calls_owned: Vec<_> = tool_calls.iter().cloned().cloned().collect();
        let assistant_tool_msg = genai::chat::ChatMessage::from(tool_calls_owned.clone());
        messages.push(assistant_tool_msg);

        let tool_responses = executor.execute_tool_calls(&tool_calls_owned).await;

        for tool_response in tool_responses {
            let tool_msg = genai::chat::ChatMessage::from(tool_response);
            messages.push(tool_msg);
        }
    }

    Err(anyhow::anyhow!(
        "Tool execution loop exceeded maximum iterations ({})",
        MAX_TOOL_ITERATIONS
    ))
}

fn resolve_conversation(
    config: &Config,
    conversation_manager: &ConversationManager,
    model: &str,
    api_keys: &[String],
) -> Result<(Conversation, String)> {
    // Helper to create new conversation with random API key index
    let create_new_conversation = || {
        let api_key_index = if api_keys.is_empty() {
            0
        } else {
            // Select a random key index for new conversations
            use rand::prelude::*;
            let mut rng = rand::thread_rng();
            (0..api_keys.len()).choose(&mut rng).unwrap_or(0)
        };
        Conversation::new_with_prompt(model.to_string(), &config.prompt, api_key_index)
    };

    // If --no-save flag is set, always create a new conversation with random key
    if config.no_save {
        log_info("--no-save flag set, creating new conversation with random API key");
        return Ok((create_new_conversation(), config.prompt.clone()));
    }

    if config.resume_last {
        // Resume latest conversation with -R flag
        log_info("Attempting to resume latest conversation (-R flag)");
        let conv = if let Some(conv) = conversation_manager.get_latest_conversation()? {
            log_info(&format!("Resumed conversation: {}", conv.id));
            conv
        } else {
            log_info("No previous conversations found, starting new conversation");
            create_new_conversation()
        };
        return Ok((conv, config.prompt.clone()));
    }

    match &config.resume_conversation {
        None => {
            // New conversation
            log_info("Starting new conversation");
            Ok((create_new_conversation(), config.prompt.clone()))
        }
        Some(id) if id.is_empty() => {
            // Resume latest conversation
            log_info("Attempting to resume latest conversation");
            let conv = if let Some(conv) = conversation_manager.get_latest_conversation()? {
                log_info(&format!("Resumed conversation: {}", conv.id));
                conv
            } else {
                log_info("No previous conversations found, starting new conversation");
                create_new_conversation()
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
