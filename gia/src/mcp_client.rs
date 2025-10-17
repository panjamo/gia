//! MCP (Model Context Protocol) Client Module
//!
//! This module provides functionality to connect to MCP servers, discover available tools,
//! and execute tool calls. It acts as a bridge between gia and external MCP tool servers.
//!
//! # Architecture
//!
//! The MCP client is designed to work alongside the existing provider architecture:
//! - Connects to MCP servers via stdio or TCP
//! - Discovers available tools on connection
//! - Executes tool calls with JSON parameters
//! - Returns structured results for LLM processing
//!
//! # Example Usage
//!
//! ```no_run
//! use gia::mcp_client::{McpClient, McpConfig};
//!
//! #[tokio::main]
//! async fn main() -> Result<(), McpError> {
//!     let config = McpConfig {
//!         server_name: "file".to_string(),
//!         connection_type: ConnectionType::Stdio,
//!         command: "mcp-file-server".to_string(),
//!         args: vec![],
//!     };
//!
//!     let mut client = McpClient::new(config).await?;
//!     let tools = client.list_tools().await?;
//!     println!("Available tools: {:?}", tools);
//!
//!     Ok(())
//! }
//! ```

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpStream;
use tokio::process::{Child, Command};
use tokio::time::{timeout, Duration};

/// Configuration for connecting to an MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpConfig {
    /// Server name (e.g., "file", "git", "code", "web", "system")
    pub server_name: String,
    /// Connection type (stdio or TCP)
    pub connection_type: ConnectionType,
    /// Command to execute for stdio connection, or empty for TCP
    pub command: String,
    /// Additional arguments for the command
    pub args: Vec<String>,
    /// TCP address (e.g., "localhost:3000") for TCP connections
    pub address: Option<String>,
}

/// Type of connection to the MCP server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ConnectionType {
    /// Standard input/output connection (subprocess)
    Stdio,
    /// TCP socket connection
    Tcp,
}

/// Metadata about an MCP tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolMetadata {
    /// Tool name
    pub name: String,
    /// Tool description
    pub description: String,
    /// JSON schema for input parameters
    pub input_schema: serde_json::Value,
}

/// Result from executing an MCP tool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Tool name that was executed
    pub tool_name: String,
    /// Execution success status
    pub success: bool,
    /// Result content (JSON or text)
    pub content: serde_json::Value,
    /// Optional error message
    pub error: Option<String>,
}

/// MCP Client for connecting to and interacting with MCP servers
pub struct McpClient {
    /// Configuration for this client
    config: McpConfig,
    /// Cached tool metadata
    tools_cache: HashMap<String, ToolMetadata>,
    /// Connection state
    connected: bool,
    /// Child process for stdio connections
    process: Option<Child>,
    /// TCP stream for TCP connections
    tcp_stream: Option<TcpStream>,
}

impl McpClient {
    /// Create a new MCP client with the given configuration
    ///
    /// # Arguments
    ///
    /// * `config` - Configuration for the MCP server connection
    ///
    /// # Errors
    ///
    /// Returns `McpError` if the connection fails
    pub async fn new(config: McpConfig) -> Result<Self, McpError> {
        let mut client = Self {
            config,
            tools_cache: HashMap::new(),
            connected: false,
            process: None,
            tcp_stream: None,
        };

        // Connect to the server
        client.connect().await?;

        Ok(client)
    }

    /// Connect to the MCP server
    async fn connect(&mut self) -> Result<(), McpError> {
        match self.config.connection_type {
            ConnectionType::Stdio => self.connect_stdio().await,
            ConnectionType::Tcp => self.connect_tcp().await,
        }
    }

    /// Connect to MCP server via stdio (subprocess)
    async fn connect_stdio(&mut self) -> Result<(), McpError> {
        use crate::logging::{log_error, log_info};

        log_info(&format!(
            "Connecting to MCP server '{}' via stdio: {} {:?}",
            self.config.server_name, self.config.command, self.config.args
        ));

        // Spawn the MCP server process
        let mut cmd = Command::new(&self.config.command);
        cmd.args(&self.config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit()); // Inherit stderr to see server logs

        let child = cmd.spawn().map_err(|e| {
            log_error(&format!(
                "Failed to spawn MCP server '{}': {}",
                self.config.server_name, e
            ));
            McpError::ConnectionFailed(format!("Failed to spawn process: {}", e))
        })?;

        self.process = Some(child);
        self.connected = true;

        log_info(&format!(
            "Successfully connected to MCP server '{}'",
            self.config.server_name
        ));

        Ok(())
    }

    /// Connect to MCP server via TCP
    async fn connect_tcp(&mut self) -> Result<(), McpError> {
        use crate::logging::{log_error, log_info};

        let address = self
            .config
            .address
            .as_ref()
            .ok_or_else(|| McpError::InvalidConfig("TCP address not provided".to_string()))?;

        log_info(&format!(
            "Connecting to unified MCP server at {} (category: '{}')",
            address, self.config.server_name
        ));

        // Connect to the TCP server
        let stream = TcpStream::connect(address).await.map_err(|e| {
            log_error(&format!("Failed to connect to MCP server at {}: {}", address, e));
            McpError::ConnectionFailed(format!("TCP connection failed: {}", e))
        })?;

        self.tcp_stream = Some(stream);
        self.connected = true;

        log_info(&format!(
            "Successfully connected to unified MCP server at {}",
            address
        ));

        Ok(())
    }

    /// Disconnect from the MCP server
    pub async fn disconnect(&mut self) -> Result<(), McpError> {
        use crate::logging::log_info;

        if !self.connected {
            return Ok(());
        }

        log_info(&format!(
            "Disconnecting from MCP server '{}'",
            self.config.server_name
        ));

        // Kill the child process if it exists
        if let Some(mut process) = self.process.take() {
            if let Err(e) = process.kill().await {
                return Err(McpError::Other(format!("Failed to kill process: {}", e)));
            }

            // Wait for the process to exit with timeout
            match timeout(Duration::from_secs(5), process.wait()).await {
                Ok(Ok(_)) => {
                    log_info(&format!(
                        "MCP server '{}' process terminated",
                        self.config.server_name
                    ));
                }
                Ok(Err(e)) => {
                    return Err(McpError::Other(format!("Failed to wait for process: {}", e)));
                }
                Err(_) => {
                    return Err(McpError::Timeout);
                }
            }
        }

        self.connected = false;
        Ok(())
    }

    /// List all available tools from the connected MCP server
    ///
    /// # Returns
    ///
    /// A vector of `ToolMetadata` describing each available tool
    ///
    /// # Errors
    ///
    /// Returns `McpError` if not connected or discovery fails
    pub async fn list_tools(&mut self) -> Result<Vec<ToolMetadata>, McpError> {
        use crate::logging::{log_error, log_info};

        if !self.connected {
            return Err(McpError::NotConnected);
        }

        log_info(&format!(
            "Discovering tools from MCP server '{}'",
            self.config.server_name
        ));

        // Check cache first
        if !self.tools_cache.is_empty() {
            log_info(&format!(
                "Returning {} cached tools",
                self.tools_cache.len()
            ));
            return Ok(self.tools_cache.values().cloned().collect());
        }

        // For Phase 1 MVP: Return mock tool list based on category
        // TODO: Implement actual JSON-RPC communication with unified server in Phase 2
        let mock_tools = match self.config.server_name.as_str() {
            "git" => vec![
                ToolMetadata {
                    name: "git_status".to_string(),
                    description: "Get git repository status".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
                ToolMetadata {
                    name: "git_log".to_string(),
                    description: "Show git commit history".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
                ToolMetadata {
                    name: "git_diff".to_string(),
                    description: "Show git diff".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
                ToolMetadata {
                    name: "git_branch".to_string(),
                    description: "List or manage git branches".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
                ToolMetadata {
                    name: "git_show".to_string(),
                    description: "Show git commit details".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
            ],
            "code" => vec![
                ToolMetadata {
                    name: "analyze_code".to_string(),
                    description: "Analyze code for complexity and metrics".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
                ToolMetadata {
                    name: "find_functions".to_string(),
                    description: "Find function definitions in code".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
                ToolMetadata {
                    name: "refactor_suggest".to_string(),
                    description: "Suggest code refactoring improvements".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
                ToolMetadata {
                    name: "code_metrics".to_string(),
                    description: "Calculate code metrics and statistics".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
            ],
            "web" => vec![
                ToolMetadata {
                    name: "fetch_url".to_string(),
                    description: "Fetch content from a URL".to_string(),
                    input_schema: serde_json::json!({
                        "type": "object",
                        "properties": {
                            "url": {"type": "string", "description": "URL to fetch"}
                        }
                    }),
                },
                ToolMetadata {
                    name: "scrape_page".to_string(),
                    description: "Scrape and parse web page content".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
                ToolMetadata {
                    name: "extract_links".to_string(),
                    description: "Extract links from a web page".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
                ToolMetadata {
                    name: "http_request".to_string(),
                    description: "Make custom HTTP request".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
            ],
            "system" => vec![
                ToolMetadata {
                    name: "system_info".to_string(),
                    description: "Get system information".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
                ToolMetadata {
                    name: "process_list".to_string(),
                    description: "List running processes".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
                ToolMetadata {
                    name: "disk_usage".to_string(),
                    description: "Show disk usage statistics".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
                ToolMetadata {
                    name: "memory_info".to_string(),
                    description: "Display memory usage information".to_string(),
                    input_schema: serde_json::json!({"type": "object", "properties": {}}),
                },
            ],
            _ => {
                log_error(&format!(
                    "Unknown tool category '{}'. Available: git, code, web, system",
                    self.config.server_name
                ));
                vec![]
            }
        };

        // Cache the tools
        for tool in &mock_tools {
            self.tools_cache
                .insert(tool.name.clone(), tool.clone());
        }

        log_info(&format!(
            "Discovered {} tools from server '{}'",
            mock_tools.len(),
            self.config.server_name
        ));

        Ok(mock_tools)
    }

    /// Execute a tool call with the given parameters
    ///
    /// # Arguments
    ///
    /// * `tool_name` - Name of the tool to execute
    /// * `parameters` - JSON parameters for the tool
    ///
    /// # Returns
    ///
    /// A `ToolResult` containing the execution result
    ///
    /// # Errors
    ///
    /// Returns `McpError` if not connected, tool not found, or execution fails
    pub async fn call_tool(
        &mut self,
        tool_name: &str,
        parameters: serde_json::Value,
    ) -> Result<ToolResult, McpError> {
        use crate::logging::{log_error, log_info};

        if !self.connected {
            return Err(McpError::NotConnected);
        }

        log_info(&format!(
            "Executing tool '{}' on server '{}' with params: {}",
            tool_name, self.config.server_name, parameters
        ));

        // Check if tool exists in cache
        if !self.tools_cache.contains_key(tool_name) {
            // Try to discover tools if cache is empty
            if self.tools_cache.is_empty() {
                self.list_tools().await?;
            }

            // Check again after discovery
            if !self.tools_cache.contains_key(tool_name) {
                log_error(&format!("Tool '{}' not found", tool_name));
                return Err(McpError::ToolNotFound(tool_name.to_string()));
            }
        }

        // For Phase 1 MVP: Execute mock tool calls
        // TODO: Implement actual JSON-RPC communication in Phase 2
        let result = self.execute_mock_tool(tool_name, parameters).await?;

        log_info(&format!(
            "Tool '{}' execution completed: success={}",
            tool_name, result.success
        ));

        Ok(result)
    }

    /// Execute a mock tool call (Phase 1 MVP)
    async fn execute_mock_tool(
        &self,
        tool_name: &str,
        parameters: serde_json::Value,
    ) -> Result<ToolResult, McpError> {
        use crate::logging::log_info;

        // Simulate tool execution with timeout
        let execution_timeout = Duration::from_secs(30);

        let result = timeout(execution_timeout, async {
            match (self.config.server_name.as_str(), tool_name) {
                ("file", "read_file") => {
                    let path = parameters
                        .get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| McpError::ExecutionFailed("Missing 'path' parameter".to_string()))?;

                    log_info(&format!("Mock: Reading file '{}'", path));

                    // Simulate reading a file
                    Ok(ToolResult {
                        tool_name: tool_name.to_string(),
                        success: true,
                        content: serde_json::json!({
                            "path": path,
                            "content": "[Mock file content - actual implementation pending]",
                            "size": 42
                        }),
                        error: None,
                    })
                }
                ("file", "list_files") => {
                    let path = parameters
                        .get("path")
                        .and_then(|v| v.as_str())
                        .ok_or_else(|| McpError::ExecutionFailed("Missing 'path' parameter".to_string()))?;

                    log_info(&format!("Mock: Listing files in '{}'", path));

                    // Simulate listing files
                    Ok(ToolResult {
                        tool_name: tool_name.to_string(),
                        success: true,
                        content: serde_json::json!({
                            "path": path,
                            "files": ["[Mock] file1.txt", "[Mock] file2.txt", "[Mock] file3.rs"],
                            "count": 3
                        }),
                        error: None,
                    })
                }
                _ => {
                    Err(McpError::ExecutionFailed(format!(
                        "Tool '{}' not implemented for server '{}'",
                        tool_name, self.config.server_name
                    )))
                }
            }
        })
        .await
        .map_err(|_| McpError::Timeout)??;

        Ok(result)
    }

    /// Get the server name this client is connected to
    pub fn server_name(&self) -> &str {
        &self.config.server_name
    }

    /// Check if the client is connected
    pub fn is_connected(&self) -> bool {
        self.connected
    }
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // Clean up connection on drop
        // Note: We can't call async methods in Drop, so we'll need to ensure
        // disconnect() is called explicitly before dropping
    }
}

/// Error types for MCP client operations
#[derive(Debug)]
pub enum McpError {
    /// Failed to connect to MCP server
    ConnectionFailed(String),
    /// Client is not connected
    NotConnected,
    /// Tool discovery failed
    DiscoveryFailed(String),
    /// Tool execution failed
    ExecutionFailed(String),
    /// Invalid configuration
    InvalidConfig(String),
    /// Tool not found
    ToolNotFound(String),
    /// Timeout during operation
    Timeout,
    /// Other errors
    Other(String),
}

impl fmt::Display for McpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            McpError::ConnectionFailed(msg) => {
                write!(f, "Failed to connect to MCP server: {}", msg)
            }
            McpError::NotConnected => write!(f, "Not connected to MCP server"),
            McpError::DiscoveryFailed(msg) => write!(f, "Tool discovery failed: {}", msg),
            McpError::ExecutionFailed(msg) => write!(f, "Tool execution failed: {}", msg),
            McpError::InvalidConfig(msg) => write!(f, "Invalid configuration: {}", msg),
            McpError::ToolNotFound(name) => write!(f, "Tool not found: {}", name),
            McpError::Timeout => write!(f, "Operation timed out"),
            McpError::Other(msg) => write!(f, "MCP error: {}", msg),
        }
    }
}

impl std::error::Error for McpError {}

impl From<anyhow::Error> for McpError {
    fn from(err: anyhow::Error) -> Self {
        McpError::Other(err.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_config_creation() {
        let config = McpConfig {
            server_name: "file".to_string(),
            connection_type: ConnectionType::Stdio,
            command: "mcp-file-server".to_string(),
            args: vec![],
            address: None,
        };

        assert_eq!(config.server_name, "file");
        assert_eq!(config.command, "mcp-file-server");
    }

    #[test]
    fn test_mcp_error_display() {
        let error = McpError::NotConnected;
        assert_eq!(error.to_string(), "Not connected to MCP server");

        let error = McpError::ToolNotFound("read_file".to_string());
        assert_eq!(error.to_string(), "Tool not found: read_file");
    }
}
