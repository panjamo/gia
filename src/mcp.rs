use anyhow::{Context, Result};
use async_trait::async_trait;
use log::{debug, info};
use reqwest::Client;
use serde_json::Value;
use std::collections::HashMap;
use std::process::{Command, Stdio};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout};

#[derive(Debug, Clone)]
pub struct McpServer {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub description: Option<String>,
    pub transport_type: McpTransportType,
}

#[derive(Debug, Clone)]
pub enum McpTransportType {
    Stdio,
    Http(String), // URL
}

#[derive(Debug)]
pub struct McpClient {
    servers: HashMap<String, McpServer>,
    active_connections: HashMap<String, McpConnection>,
}

#[derive(Debug)]
struct McpConnection {
    transport: Box<dyn McpTransport>,
    request_id: u64,
}

#[async_trait]
pub trait McpTransport: std::fmt::Debug + Send {
    async fn send_request(&mut self, request: Value) -> Result<()>;
    async fn read_response(&mut self) -> Result<Value>;
    async fn initialize(&mut self, client_info: Value) -> Result<()>;
    async fn shutdown(&mut self) -> Result<()>;
    
    /// For HTTP transport, send request and receive response in one call
    async fn send_and_receive(&mut self, request: Value) -> Result<Value> {
        self.send_request(request).await?;
        self.read_response().await
    }
}

#[derive(Debug)]
struct StdioTransport {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout_reader: BufReader<ChildStdout>,
}

#[derive(Debug)]
struct HttpTransport {
    client: Client,
    url: String,
}

#[async_trait]
impl McpTransport for StdioTransport {
    async fn send_request(&mut self, request: Value) -> Result<()> {
        let request_str = format!("{}\n", serde_json::to_string(&request)?);
        
        if let Some(stdin) = &mut self.stdin {
            stdin.write_all(request_str.as_bytes()).await
                .context("Failed to write to MCP server stdin")?;
            stdin.flush().await
                .context("Failed to flush MCP server stdin")?;
        }
        
        debug!("Sent MCP request via stdio: {}", request_str.trim());
        Ok(())
    }

    async fn read_response(&mut self) -> Result<Value> {
        let mut line = String::new();
        self.stdout_reader
            .read_line(&mut line)
            .await
            .context("Failed to read from MCP server stdout")?;

        debug!("Received MCP response via stdio: {}", line.trim());
        
        let response: Value = serde_json::from_str(&line.trim())
            .context("Failed to parse JSON response from MCP server")?;

        Ok(response)
    }

    async fn initialize(&mut self, client_info: Value) -> Result<()> {
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "clientInfo": client_info
            }
        });

        self.send_request(init_request).await?;
        let response = self.read_response().await?;
        
        debug!("Initialize response: {}", serde_json::to_string_pretty(&response).unwrap_or_default());
        
        if let Some(error) = response.get("error") {
            return Err(anyhow::anyhow!("MCP initialization failed: {}", error));
        }
        
        // Send initialized notification
        let initialized_notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        self.send_request(initialized_notification).await?;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        if let Some(mut stdin) = self.stdin.take() {
            let _ = stdin.shutdown().await;
        }
        let _ = self.child.wait().await;
        Ok(())
    }
}

#[async_trait]
impl McpTransport for HttpTransport {
    async fn send_request(&mut self, request: Value) -> Result<()> {
        // For HTTP transport, we don't send immediately, we'll combine send_request and read_response
        // This is a limitation of HTTP vs streaming protocols
        Ok(())
    }

    async fn read_response(&mut self) -> Result<Value> {
        // This will be called after send_request, but HTTP is request-response so we can't separate them
        Err(anyhow::anyhow!("HTTP transport requires send_and_receive method"))
    }

    async fn initialize(&mut self, client_info: Value) -> Result<()> {
        let init_request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2024-11-05",
                "capabilities": {
                    "tools": {}
                },
                "clientInfo": client_info
            }
        });

        let response = self.send_and_receive_impl(init_request).await?;
        
        debug!("Initialize response: {}", serde_json::to_string_pretty(&response).unwrap_or_default());
        
        if let Some(error) = response.get("error") {
            return Err(anyhow::anyhow!("MCP initialization failed: {}", error));
        }

        // Send initialized notification
        let initialized_notification = serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        });

        let _response = self.send_and_receive_impl(initialized_notification).await?;
        Ok(())
    }

    async fn shutdown(&mut self) -> Result<()> {
        // HTTP connections are stateless, nothing to shutdown
        Ok(())
    }

    async fn send_and_receive(&mut self, request: Value) -> Result<Value> {
        self.send_and_receive_impl(request).await
    }
}

impl HttpTransport {
    async fn send_and_receive_impl(&self, request: Value) -> Result<Value> {
        debug!("Sending HTTP MCP request to {}: {}", self.url, serde_json::to_string_pretty(&request).unwrap_or_default());
        
        let response = self.client
            .post(&self.url)
            .header("Content-Type", "application/json")
            .json(&request)
            .send()
            .await
            .with_context(|| format!("Failed to send HTTP request to {}", self.url))?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!("HTTP request failed with status: {}", response.status()));
        }

        let response_json: Value = response.json().await
            .context("Failed to parse HTTP response as JSON")?;

        debug!("Received HTTP MCP response: {}", serde_json::to_string_pretty(&response_json).unwrap_or_default());
        
        Ok(response_json)
    }
}

impl McpClient {
    pub fn new() -> Self {
        Self {
            servers: HashMap::new(),
            active_connections: HashMap::new(),
        }
    }

    /// Add a new MCP server configuration
    pub fn add_server(&mut self, server: McpServer) {
        info!("Adding MCP server: {}", server.name);
        self.servers.insert(server.name.clone(), server);
    }

    /// Connect to an MCP server
    pub async fn connect(&mut self, server_name: &str) -> Result<()> {
        let server = self
            .servers
            .get(server_name)
            .context("Server not found")?
            .clone();

        info!("Connecting to MCP server: {} via {:?}", server_name, server.transport_type);

        let transport: Box<dyn McpTransport> = match &server.transport_type {
            McpTransportType::Stdio => {
                let mut cmd = Command::new(&server.command);
                cmd.args(&server.args)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped());

                let mut child = tokio::process::Command::from(cmd)
                    .spawn()
                    .with_context(|| format!("Failed to start MCP server: {}", server_name))?;

                let stdin = child.stdin.take().context("Failed to get stdin")?;
                let stdout = child.stdout.take().context("Failed to get stdout")?;
                let stdout_reader = BufReader::new(stdout);

                Box::new(StdioTransport {
                    child,
                    stdin: Some(stdin),
                    stdout_reader,
                })
            }
            McpTransportType::Http(url) => {
                let client = Client::new();
                Box::new(HttpTransport {
                    client,
                    url: url.clone(),
                })
            }
        };

        let mut connection = McpConnection {
            transport,
            request_id: 0,
        };

        // Initialize the connection
        let client_info = serde_json::json!({
            "name": "gia",
            "version": "0.1.0"
        });
        
        connection.transport.initialize(client_info).await?;

        self.active_connections.insert(server_name.to_string(), connection);

        info!("MCP connection initialized for server: {}", server_name);
        Ok(())
    }

    /// Send request and receive response (unified interface for both transports)
    async fn send_request_and_receive(&mut self, server_name: &str, request: Value) -> Result<Value> {
        let connection = self
            .active_connections
            .get_mut(server_name)
            .context("Connection not found")?;
        
        connection.transport.send_and_receive(request).await
    }

    /// List available tools from an MCP server
    pub async fn list_tools(&mut self, server_name: &str) -> Result<Vec<McpTool>> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.next_request_id(server_name),
            "method": "tools/list"
        });

        let response = self.send_request_and_receive(server_name, request).await?;
        
        debug!("Tools list response: {}", serde_json::to_string_pretty(&response).unwrap_or_default());
        
        // Check if response has an error
        if let Some(error) = response.get("error") {
            return Err(anyhow::anyhow!("Tools list failed: {}", error));
        }
        
        // Try to extract tools from the result
        let result = response.get("result")
            .context("Response missing result field")?;
        
        // Try different possible field names for tools
        let tools_value = result.get("tools")
            .or_else(|| result.get("result"))  // Some servers might nest it differently
            .context("Response missing tools field in result")?;
        
        // Handle case where tools might be directly in result instead of nested
        let final_tools_value = if tools_value.is_array() {
            tools_value
        } else if let Some(nested_tools) = tools_value.get("tools") {
            nested_tools
        } else {
            tools_value
        };
        
        let tools: Vec<McpTool> = serde_json::from_value(final_tools_value.clone())
            .with_context(|| format!("Failed to parse tools list. Full response: {}\nTools value: {}", 
                serde_json::to_string_pretty(&response).unwrap_or_default(),
                serde_json::to_string_pretty(&final_tools_value).unwrap_or_default()))?;
        
        Ok(tools)
    }

    /// Call an MCP tool
    pub async fn call_tool(
        &mut self,
        server_name: &str,
        tool_name: &str,
        arguments: Value,
    ) -> Result<Value> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.next_request_id(server_name),
            "method": "tools/call",
            "params": {
                "name": tool_name,
                "arguments": arguments
            }
        });

        let response = self.send_request_and_receive(server_name, request).await?;
        
        if let Some(error) = response.get("error") {
            return Err(anyhow::anyhow!("Tool call failed: {}", error));
        }
        
        Ok(response["result"].clone())
    }

    /// List available resources from an MCP server
    pub async fn list_resources(&mut self, server_name: &str) -> Result<Vec<McpResource>> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.next_request_id(server_name),
            "method": "resources/list"
        });

        let response = self.send_request_and_receive(server_name, request).await?;
        
        let resources: Vec<McpResource> = serde_json::from_value(
            response["result"]["resources"].clone()
        ).context("Failed to parse resources list")?;
        
        Ok(resources)
    }

    /// Read a resource from an MCP server
    pub async fn read_resource(&mut self, server_name: &str, uri: &str) -> Result<String> {
        let request = serde_json::json!({
            "jsonrpc": "2.0",
            "id": self.next_request_id(server_name),
            "method": "resources/read",
            "params": {
                "uri": uri
            }
        });

        let response = self.send_request_and_receive(server_name, request).await?;
        
        if let Some(error) = response.get("error") {
            return Err(anyhow::anyhow!("Resource read failed: {}", error));
        }
        
        let content = response["result"]["contents"][0]["text"]
            .as_str()
            .context("Failed to extract resource content")?;
        
        Ok(content.to_string())
    }



    /// Get next request ID for a server
    fn next_request_id(&mut self, server_name: &str) -> u64 {
        if let Some(connection) = self.active_connections.get_mut(server_name) {
            connection.request_id += 1;
            connection.request_id
        } else {
            1
        }
    }

    /// Disconnect from an MCP server
    pub async fn disconnect(&mut self, server_name: &str) -> Result<()> {
        if let Some(mut connection) = self.active_connections.remove(server_name) {
            info!("Disconnecting from MCP server: {}", server_name);
            connection.transport.shutdown().await?;
        }
        Ok(())
    }

    /// Disconnect from all servers
    pub async fn disconnect_all(&mut self) -> Result<()> {
        let server_names: Vec<String> = self.active_connections.keys().cloned().collect();
        for server_name in server_names {
            self.disconnect(&server_name).await?;
        }
        Ok(())
    }

    /// Get list of configured servers
    pub fn get_servers(&self) -> Vec<&McpServer> {
        self.servers.values().collect()
    }

    /// Get list of connected servers
    pub fn get_connected_servers(&self) -> Vec<&str> {
        self.active_connections.keys().map(|s| s.as_str()).collect()
    }
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    #[serde(alias = "inputSchema", alias = "schema")]
    pub input_schema: Value,
}

#[derive(Debug, Clone, serde::Deserialize, serde::Serialize)]
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(rename = "mimeType")]
    pub mime_type: Option<String>,
}

impl Drop for McpClient {
    fn drop(&mut self) {
        // Note: We can't await in drop, so we just terminate connections forcefully
        for (server_name, _connection) in self.active_connections.drain() {
            debug!("Terminating MCP server connection: {}", server_name);
            // Transport cleanup will happen when connection is dropped
        }
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        // Kill the child process if it's still running
        let _ = self.child.kill();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mcp_server_creation() {
        let server = McpServer {
            name: "test-server".to_string(),
            command: "echo".to_string(),
            args: vec!["hello".to_string()],
            description: Some("Test server".to_string()),
        };

        assert_eq!(server.name, "test-server");
        assert_eq!(server.command, "echo");
        assert_eq!(server.args, vec!["hello"]);
    }

    #[test]
    fn test_mcp_client_creation() {
        let client = McpClient::new();
        assert!(client.servers.is_empty());
        assert!(client.active_connections.is_empty());
    }
}