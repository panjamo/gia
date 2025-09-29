use anyhow::{Context, Result};

use crate::cli::Config;
use crate::constants::get_context_window_limit;
use crate::conversation::{Conversation, ConversationManager, MessageRole};
use crate::input::{get_input_text, validate_image_sources};
use crate::logging::{log_error, log_info};
use crate::mcp::{McpClient, McpServer, McpTransportType};
use crate::output::output_text;
use crate::provider::{ProviderConfig, ProviderFactory};

pub async fn run_app(mut config: Config) -> Result<()> {
    // Initialize MCP client if servers are configured
    let mut mcp_client = if !config.mcp_servers.is_empty() || config.list_mcp_tools || config.mcp_tool_call.is_some() {
        Some(initialize_mcp_client(&config).await?)
    } else {
        None
    };

    // Handle MCP-specific commands
    if config.list_mcp_tools {
        return handle_list_mcp_tools(&mut mcp_client).await;
    }

    if let Some((server, tool, args)) = &config.mcp_tool_call {
        return handle_mcp_tool_call(&mut mcp_client, server, tool, args).await;
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

    // Generate content using ordered content sources
    if config.ordered_content.is_empty() {
        log_info(&format!(
            "Sending text request to {} API using model: {}",
            provider.provider_name(),
            provider.model_name()
        ));
        // For backwards compatibility, if no ordered content, use the old method
        let response = provider
            .generate_content_with_image_sources(&context_prompt, &config.image_sources)
            .await
            .context("Failed to generate content")?;

        // Add messages to conversation and save
        conversation.add_message(MessageRole::User, input_text);
        conversation.add_message(MessageRole::Assistant, response.clone());
        conversation_manager
            .save_conversation(&conversation)
            .context("Failed to save conversation")?;

        // Output response
        output_text(&response, &config).context("Failed to output response")?;
        log_info("Successfully completed request");
        return Ok(());
    }
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

async fn initialize_mcp_client(config: &Config) -> Result<McpClient> {
    let mut mcp_client = McpClient::new();
    
    for server_config in &config.mcp_servers {
        let server = parse_mcp_server_config(server_config)?;
        mcp_client.add_server(server.clone());
        
        log_info(&format!("Connecting to MCP server: {}", server.name));
        match mcp_client.connect(&server.name).await {
            Ok(_) => {
                log_info(&format!("Successfully connected to MCP server: {}", server.name));
            }
            Err(e) => {
                // Check if it's a connection error
                let error_msg = e.to_string();
                if error_msg.contains("No connection could be made") || 
                   error_msg.contains("Connection refused") ||
                   error_msg.contains("tcp connect error") ||
                   error_msg.contains("target machine actively refused") ||
                   error_msg.contains("Failed to send HTTP request") {
                    eprintln!("Warning: Cannot connect to MCP server '{}' - server may not be running", server.name);
                    eprintln!("  Server config: {}", format_server_info(&server));
                    eprintln!("  Skipping this server and continuing...");
                    continue;
                } else {
                    return Err(e).with_context(|| format!("Failed to connect to MCP server: {}", server.name));
                }
            }
        }
    }
    
    Ok(mcp_client)
}

fn format_server_info(server: &McpServer) -> String {
    match &server.transport_type {
        McpTransportType::Stdio => {
            if server.args.is_empty() {
                format!("stdio:{}", server.command)
            } else {
                format!("stdio:{}:{}", server.command, server.args.join(","))
            }
        }
        McpTransportType::Http(url) => format!("http:{}", url),
    }
}

fn parse_mcp_server_config(config: &str) -> Result<McpServer> {
    // Find the first colon to separate name from command+args
    let first_colon = config.find(':').context("Invalid MCP server config format. Use: name:command[:args] or name:http://host:port")?;
    
    let name = config[..first_colon].to_string();
    let command_and_args = &config[first_colon + 1..];
    
    // Check if this is an HTTP URL
    if command_and_args.starts_with("http://") || command_and_args.starts_with("https://") {
        return Ok(McpServer {
            name,
            command: command_and_args.to_string(),
            args: Vec::new(),
            description: None,
            transport_type: McpTransportType::Http(command_and_args.to_string()),
        });
    }
    
    // For args separation, we need to be more careful with Windows paths
    // Look for a colon that is NOT immediately after a single letter (drive letter)
    let mut args_split_pos = None;
    let chars: Vec<char> = command_and_args.chars().collect();
    
    for (i, &ch) in chars.iter().enumerate() {
        if ch == ':' {
            // Check if this colon is part of a Windows drive path (X:\)
            // A drive colon should be at position 1 and followed by \ or /
            if i == 1 && (i + 1 < chars.len() && (chars[i + 1] == '\\' || chars[i + 1] == '/')) {
                // This is a drive letter colon, skip it
                continue;
            }
            // This is our args separator
            args_split_pos = Some(i);
            break;
        }
    }
    
    if let Some(split_pos) = args_split_pos {
        let command = command_and_args[..split_pos].to_string();
        let args = command_and_args[split_pos + 1..]
            .split(',')
            .map(|s| s.trim().to_string())
            .collect();
        
        Ok(McpServer {
            name,
            command,
            args,
            description: None,
            transport_type: McpTransportType::Stdio,
        })
    } else {
        // No args, just command
        Ok(McpServer {
            name,
            command: command_and_args.to_string(),
            args: Vec::new(),
            description: None,
            transport_type: McpTransportType::Stdio,
        })
    }
}

async fn handle_list_mcp_tools(mcp_client: &mut Option<McpClient>) -> Result<()> {
    let Some(client) = mcp_client else {
        println!("No MCP servers configured. Use --mcp-server to add servers.");
        return Ok(());
    };
    
    println!("Available MCP Tools:");
    println!("===================");
    
    let server_names: Vec<String> = client.get_connected_servers().iter().map(|s| s.to_string()).collect();
    for server_name in server_names {
        println!("\nServer: {}", server_name);
        match client.list_tools(&server_name).await {
            Ok(tools) => {
                if tools.is_empty() {
                    println!("  No tools available");
                } else {
                    for tool in tools {
                        println!("  - {}", tool.name);
                        if let Some(description) = &tool.description {
                            println!("    Description: {}", description);
                        }
                        println!("    Schema: {}", serde_json::to_string_pretty(&tool.input_schema)?);
                    }
                }
            }
            Err(e) => {
                println!("  Error listing tools: {}", e);
            }
        }
    }
    
    Ok(())
}

async fn handle_mcp_tool_call(
    mcp_client: &mut Option<McpClient>, 
    server: &str, 
    tool: &str, 
    args_json: &str
) -> Result<()> {
    let Some(client) = mcp_client else {
        return Err(anyhow::anyhow!("No MCP client available"));
    };
    
    let arguments: serde_json::Value = serde_json::from_str(args_json)
        .context("Failed to parse tool arguments as JSON")?;
    
    log_info(&format!("Calling MCP tool: {}:{} with args: {}", server, tool, args_json));
    
    match client.call_tool(server, tool, arguments).await {
        Ok(result) => {
            println!("Tool call result:");
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        Err(e) => {
            return Err(anyhow::anyhow!("Tool call failed: {}", e));
        }
    }
    
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

    // Generate markdown content
    let markdown_content = conversation.format_as_chat_markdown();

    // Create a temporary config for output with proper prompt for filename generation
    let mut output_config = config.clone();
    output_config.prompt = format!("conversation_{}", conversation.id);

    // Use the normal output system
    output_text(&markdown_content, &output_config).context("Failed to output conversation")?;

    Ok(())
}
