/// Tool implementations
///
/// Each tool follows the KISS principle: simple, focused implementation.
/// All tools use the same pattern:
/// 1. Parse arguments from JSON
/// 2. Validate with SecurityContext
/// 3. Execute operation
/// 4. Return result as string
use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::io::{self, Write};
use std::time::Duration;
use tokio::process::Command;

use super::registry::GiaTool;
use super::security::SecurityContext;

// ============================================================================
// ReadFileTool
// ============================================================================

#[derive(Serialize, Deserialize)]
struct ReadFileArgs {
    filepath: String,
}

pub struct ReadFileTool;

#[async_trait]
impl GiaTool for ReadFileTool {
    fn name(&self) -> &str {
        "read_file"
    }

    fn description(&self) -> &str {
        "Read the contents of a text file from the filesystem. \
         Returns the file content as a string. \
         Use this when you need to examine file contents."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filepath": {
                    "type": "string",
                    "description": "The path to the file to read (absolute or relative to current directory)"
                }
            },
            "required": ["filepath"]
        })
    }

    async fn execute(&self, args: Value, context: &SecurityContext) -> Result<String> {
        let args: ReadFileArgs =
            serde_json::from_value(args).context("Invalid arguments for read_file")?;

        let path = std::path::Path::new(&args.filepath);

        // Security check
        if !context.is_path_allowed(path) {
            return Err(anyhow!(
                "Access denied: {} is outside allowed directories",
                args.filepath
            ));
        }

        // Size check
        let metadata = tokio::fs::metadata(path)
            .await
            .context(format!("Failed to access file: {}", args.filepath))?;

        if metadata.len() > context.max_file_size() as u64 {
            return Err(anyhow!(
                "File too large: {} bytes (max: {})",
                metadata.len(),
                context.max_file_size()
            ));
        }

        // Read file
        let content = tokio::fs::read_to_string(path)
            .await
            .context(format!("Failed to read file: {}", args.filepath))?;

        Ok(format!("Contents of {}:\n\n{}", args.filepath, content))
    }
}

// ============================================================================
// WriteFileTool
// ============================================================================

#[derive(Serialize, Deserialize)]
struct WriteFileArgs {
    filepath: String,
    content: String,
}

pub struct WriteFileTool;

#[async_trait]
impl GiaTool for WriteFileTool {
    fn name(&self) -> &str {
        "write_file"
    }

    fn description(&self) -> &str {
        "Write content to a file on the filesystem. \
         Creates the file if it doesn't exist, overwrites if it does. \
         Use this when you need to save or update file contents."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "filepath": {
                    "type": "string",
                    "description": "The path where to write the file"
                },
                "content": {
                    "type": "string",
                    "description": "The content to write to the file"
                }
            },
            "required": ["filepath", "content"]
        })
    }

    async fn execute(&self, args: Value, context: &SecurityContext) -> Result<String> {
        let args: WriteFileArgs =
            serde_json::from_value(args).context("Invalid arguments for write_file")?;

        let path = std::path::Path::new(&args.filepath);

        // Security check
        if !context.is_path_allowed(path) {
            return Err(anyhow!(
                "Access denied: {} is outside allowed directories",
                args.filepath
            ));
        }

        // Create parent directories if needed
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .context("Failed to create parent directories")?;
        }

        // Write file
        tokio::fs::write(path, &args.content)
            .await
            .context(format!("Failed to write file: {}", args.filepath))?;

        Ok(format!(
            "Successfully wrote {} bytes to {}",
            args.content.len(),
            args.filepath
        ))
    }
}

// ============================================================================
// ListDirectoryTool
// ============================================================================

#[derive(Serialize, Deserialize)]
struct ListDirectoryArgs {
    #[serde(default = "default_path")]
    path: String,
}

fn default_path() -> String {
    ".".to_string()
}

pub struct ListDirectoryTool;

#[async_trait]
impl GiaTool for ListDirectoryTool {
    fn name(&self) -> &str {
        "list_directory"
    }

    fn description(&self) -> &str {
        "List files and directories in a given path. \
         Returns a list of entries with file/directory indicators. \
         Use this to explore directory structures."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The directory path to list (defaults to current directory if not specified)"
                }
            }
        })
    }

    async fn execute(&self, args: Value, context: &SecurityContext) -> Result<String> {
        let args: ListDirectoryArgs = serde_json::from_value(args).unwrap_or(ListDirectoryArgs {
            path: ".".to_string(),
        });

        let path = std::path::Path::new(&args.path);

        // Security check
        if !context.is_path_allowed(path) {
            return Err(anyhow!(
                "Access denied: {} is outside allowed directories",
                args.path
            ));
        }

        let mut entries = Vec::new();
        let mut dir_reader = tokio::fs::read_dir(path)
            .await
            .context(format!("Failed to read directory: {}", args.path))?;

        while let Some(entry) = dir_reader
            .next_entry()
            .await
            .context("Failed to read directory entry")?
        {
            let metadata = entry.metadata().await?;
            let prefix = if metadata.is_dir() { "📁" } else { "📄" };
            let name = entry.file_name().to_string_lossy().to_string();
            entries.push(format!("{} {}", prefix, name));
        }

        entries.sort();

        Ok(format!(
            "Contents of {}:\n\n{}",
            args.path,
            entries.join("\n")
        ))
    }
}

// ============================================================================
// SearchWebTool
// ============================================================================

#[derive(Debug)]
enum SearchProvider {
    DuckDuckGo,
    Brave { api_key: String },
}

impl SearchProvider {
    fn from_env() -> Result<Self> {
        match std::env::var("GIA_SEARCH_API").ok().as_deref() {
            Some("brave") => {
                let api_key = std::env::var("GIA_BRAVE_API_KEY")
                    .context("GIA_BRAVE_API_KEY not set (required for Brave search)")?;
                Ok(Self::Brave { api_key })
            }
            _ => Ok(Self::DuckDuckGo),
        }
    }

    async fn search(&self, query: &str) -> Result<String> {
        match self {
            Self::DuckDuckGo => search_duckduckgo(query).await,
            Self::Brave { api_key } => search_brave(query, api_key).await,
        }
    }
}

#[derive(Deserialize)]
struct DuckDuckGoResponse {
    #[serde(rename = "AbstractText")]
    abstract_text: String,
    #[serde(rename = "AbstractURL")]
    abstract_url: String,
    #[serde(rename = "RelatedTopics")]
    related_topics: Vec<RelatedTopic>,
    #[serde(rename = "Heading")]
    heading: String,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RelatedTopic {
    Topic {
        #[serde(rename = "Text")]
        text: String,
        #[serde(rename = "FirstURL")]
        first_url: String,
    },
    Topics {
        #[serde(rename = "Topics")]
        topics: Vec<RelatedTopic>,
    },
}

#[derive(Deserialize)]
struct BraveResponse {
    web: BraveWebResults,
}

#[derive(Deserialize)]
struct BraveWebResults {
    results: Vec<BraveResult>,
}

#[derive(Deserialize)]
struct BraveResult {
    title: String,
    description: String,
    url: String,
}

async fn search_duckduckgo(query: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("gia-cli/1.0")
        .build()
        .context("Failed to create HTTP client")?;

    let url = format!(
        "https://api.duckduckgo.com/?q={}&format=json",
        urlencoding::encode(query)
    );

    let response = client
        .get(&url)
        .send()
        .await
        .context("Failed to send search request")?;

    if !response.status().is_success() {
        return Err(anyhow!("Search failed with HTTP {}", response.status()));
    }

    let body_bytes = response
        .bytes()
        .await
        .context("Failed to read response body")?;

    if body_bytes.len() > 2 * 1024 * 1024 {
        return Err(anyhow!("Search response too large"));
    }

    let search_result: DuckDuckGoResponse = serde_json::from_slice(&body_bytes)
        .context("Failed to parse search results")?;

    Ok(format_duckduckgo_results(&search_result, query))
}

async fn search_brave(query: &str, api_key: &str) -> Result<String> {
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .user_agent("gia-cli/1.0")
        .build()
        .context("Failed to create HTTP client")?;

    let url = format!(
        "https://api.search.brave.com/res/v1/web/search?q={}",
        urlencoding::encode(query)
    );

    let response = client
        .get(&url)
        .header("X-Subscription-Token", api_key)
        .send()
        .await
        .context("Failed to send search request")?;

    if !response.status().is_success() {
        return Err(anyhow!("Search failed with HTTP {}", response.status()));
    }

    let body_bytes = response
        .bytes()
        .await
        .context("Failed to read response body")?;

    if body_bytes.len() > 2 * 1024 * 1024 {
        return Err(anyhow!("Search response too large"));
    }

    let search_result: BraveResponse = serde_json::from_slice(&body_bytes)
        .context("Failed to parse search results")?;

    Ok(format_brave_results(&search_result, query))
}

fn format_duckduckgo_results(response: &DuckDuckGoResponse, query: &str) -> String {
    let mut result = format!("Search results for '{}':\n\n", query);

    if !response.heading.is_empty() {
        result.push_str(&format!("## {}\n\n", response.heading));
    }

    if !response.abstract_text.is_empty() {
        result.push_str(&format!("{}\n", response.abstract_text));
        if !response.abstract_url.is_empty() {
            result.push_str(&format!("Source: {}\n\n", response.abstract_url));
        }
    }

    if !response.related_topics.is_empty() {
        result.push_str("### Related Information:\n\n");
        let mut count = 0;
        for topic in &response.related_topics {
            if count >= 5 {
                break;
            }
            if let Some((text, url)) = extract_topic_info(topic) {
                count += 1;
                result.push_str(&format!("{}. {}\n   {}\n\n", count, text, url));
            }
        }
    }

    if response.abstract_text.is_empty() && response.related_topics.is_empty() {
        result.push_str("No detailed results found. Try different keywords.\n");
    }

    result
}

fn format_brave_results(response: &BraveResponse, query: &str) -> String {
    let mut result = format!("Search results for '{}':\n\n", query);

    if response.web.results.is_empty() {
        result.push_str("No search results found. Try different keywords.\n");
        return result;
    }

    result.push_str("### Top Results:\n\n");
    for (i, res) in response.web.results.iter().take(5).enumerate() {
        result.push_str(&format!("{}. {}\n", i + 1, res.title));
        if !res.description.is_empty() {
            result.push_str(&format!("   {}\n", res.description));
        }
        result.push_str(&format!("   {}\n\n", res.url));
    }

    result
}

fn extract_topic_info(topic: &RelatedTopic) -> Option<(String, String)> {
    match topic {
        RelatedTopic::Topic { text, first_url } => {
            if !text.is_empty() && !first_url.is_empty() {
                Some((text.clone(), first_url.clone()))
            } else {
                None
            }
        }
        RelatedTopic::Topics { topics } => topics.first().and_then(extract_topic_info),
    }
}

#[derive(Serialize, Deserialize)]
struct SearchWebArgs {
    query: String,
}

pub struct SearchWebTool;

#[async_trait]
impl GiaTool for SearchWebTool {
    fn name(&self) -> &str {
        "search_web"
    }

    fn description(&self) -> &str {
        "Search the web for information using a search query. \
         Returns a summary of search results. \
         Use this when you need current information or web resources."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to execute"
                }
            },
            "required": ["query"]
        })
    }

    async fn execute(&self, args: Value, context: &SecurityContext) -> Result<String> {
        if !context.is_web_search_allowed() {
            return Err(anyhow!("Web search is disabled"));
        }

        let args: SearchWebArgs =
            serde_json::from_value(args).context("Invalid arguments for search_web")?;

        if args.query.len() > 500 {
            return Err(anyhow!("Search query too long (max 500 characters)"));
        }

        let provider = SearchProvider::from_env()?;
        provider.search(&args.query).await
    }
}

// ============================================================================
// ExecuteCommandTool
// ============================================================================

#[derive(Serialize, Deserialize)]
struct ExecuteCommandArgs {
    command: String,
    #[serde(default)]
    working_directory: Option<String>,
}

pub struct ExecuteCommandTool;

impl ExecuteCommandTool {
    /// Get default shell for the platform
    fn get_default_shell() -> (&'static str, &'static str) {
        #[cfg(target_os = "windows")]
        return ("cmd", "/C");

        #[cfg(not(target_os = "windows"))]
        {
            // Try to detect user's shell
            if let Ok(shell) = std::env::var("SHELL") {
                if shell.contains("zsh") {
                    return ("zsh", "-c");
                } else if shell.contains("fish") {
                    return ("fish", "-c");
                }
            }
            ("bash", "-c")
        }
    }

    /// Request user confirmation for command execution
    fn request_confirmation(command: &str, working_dir: &str, timeout_secs: u64) -> Result<bool> {
        eprintln!("\n🔧 AI wants to execute command:\n");
        eprintln!("Command: {}", command);
        eprintln!("Working directory: {}", working_dir);
        eprintln!("Timeout: {}s\n", timeout_secs);
        eprint!("Allow this command? [y/N] ");
        io::stderr().flush()?;

        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        Ok(input.trim().to_lowercase() == "y")
    }
}

#[async_trait]
impl GiaTool for ExecuteCommandTool {
    fn name(&self) -> &str {
        "execute_command"
    }

    fn description(&self) -> &str {
        "Execute a shell command (bash, cmd, powershell, zsh). \
         Use this to run command-line tools like git, gh, npm, cargo, etc. \
         Returns stdout/stderr and exit code."
    }

    fn schema(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "command": {
                    "type": "string",
                    "description": "The command to execute (e.g., 'git status', 'gh pr list', 'npm test')"
                },
                "working_directory": {
                    "type": "string",
                    "description": "Optional working directory for command execution (defaults to allowed directory)"
                }
            },
            "required": ["command"]
        })
    }

    async fn execute(&self, args: Value, context: &SecurityContext) -> Result<String> {
        if !context.is_command_execution_allowed() {
            return Err(anyhow!(
                "Command execution is disabled. Use --allow-command-execution to enable."
            ));
        }

        let args: ExecuteCommandArgs =
            serde_json::from_value(args).context("Invalid arguments for execute_command")?;

        // Validate command against blocklist
        if !context.is_command_allowed(&args.command) {
            return Err(anyhow!(
                "Command blocked for security reasons: {}",
                args.command
            ));
        }

        // Determine working directory
        let working_dir = if let Some(ref dir) = args.working_directory {
            std::path::PathBuf::from(dir)
        } else {
            std::env::current_dir()?
        };

        // Validate working directory is allowed
        if !context.is_path_allowed(&working_dir) {
            return Err(anyhow!(
                "Access denied: {} is outside allowed directories",
                working_dir.display()
            ));
        }

        // Request user confirmation if required
        if context.requires_command_confirmation() {
            if !Self::request_confirmation(
                &args.command,
                &working_dir.to_string_lossy(),
                context.command_timeout().as_secs(),
            )? {
                return Ok("Command execution cancelled by user".to_string());
            }
        }

        // Get shell
        let (shell, shell_flag) = Self::get_default_shell();

        // Execute command with timeout
        let output = tokio::time::timeout(
            context.command_timeout(),
            Command::new(shell)
                .args(&[shell_flag, &args.command])
                .current_dir(&working_dir)
                .output(),
        )
        .await
        .context("Command execution timed out")?
        .context("Failed to execute command")?;

        // Format output
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let exit_code = output.status.code().unwrap_or(-1);

        let mut result = format!("Command: {}\n", args.command);
        result.push_str(&format!("Working directory: {}\n", working_dir.display()));
        result.push_str(&format!("Exit code: {}\n\n", exit_code));

        if !stdout.is_empty() {
            result.push_str("=== STDOUT ===\n");
            result.push_str(&stdout);
            result.push('\n');
        }

        if !stderr.is_empty() {
            result.push_str("=== STDERR ===\n");
            result.push_str(&stderr);
            result.push('\n');
        }

        if stdout.is_empty() && stderr.is_empty() {
            result.push_str("(No output)\n");
        }

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_read_file_success() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("test.txt");
        tokio::fs::write(&file_path, "test content").await.unwrap();

        let tool = ReadFileTool;
        let args = json!({ "filepath": file_path.to_string_lossy() });
        let context = SecurityContext::new().with_allowed_dir(temp_dir.path());

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());
        assert!(result.unwrap().contains("test content"));
    }

    #[tokio::test]
    async fn test_read_file_denied() {
        let tool = ReadFileTool;
        let args = json!({ "filepath": "/etc/passwd" });
        let context = SecurityContext::new().with_allowed_dir(env::temp_dir());

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Access denied"));
    }

    #[tokio::test]
    async fn test_write_file_success() {
        let temp_dir = TempDir::new().unwrap();
        let file_path = temp_dir.path().join("new_file.txt");

        let tool = WriteFileTool;
        let args = json!({
            "filepath": file_path.to_string_lossy(),
            "content": "hello world"
        });
        let context = SecurityContext::new().with_allowed_dir(temp_dir.path());

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());

        // Verify file was written
        let content = tokio::fs::read_to_string(&file_path).await.unwrap();
        assert_eq!(content, "hello world");
    }

    #[tokio::test]
    async fn test_list_directory_success() {
        let temp_dir = TempDir::new().unwrap();
        tokio::fs::write(temp_dir.path().join("file1.txt"), "content")
            .await
            .unwrap();
        tokio::fs::write(temp_dir.path().join("file2.txt"), "content")
            .await
            .unwrap();

        let tool = ListDirectoryTool;
        let args = json!({ "path": temp_dir.path().to_string_lossy() });
        let context = SecurityContext::new().with_allowed_dir(temp_dir.path());

        let result = tool.execute(args, &context).await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert!(output.contains("file1.txt"));
        assert!(output.contains("file2.txt"));
    }

    #[tokio::test]
    async fn test_command_blocklist() {
        let context = SecurityContext::new();
        assert!(!context.is_command_allowed("rm -rf /"));
        assert!(!context.is_command_allowed("dd if=/dev/zero"));
        assert!(context.is_command_allowed("ls -la"));
        assert!(context.is_command_allowed("git status"));
    }

    #[test]
    #[serial_test::serial]
    fn test_search_provider_default() {
        unsafe {
            env::remove_var("GIA_SEARCH_API");
            env::remove_var("GIA_BRAVE_API_KEY");
        }
        let provider = SearchProvider::from_env().unwrap();
        assert!(matches!(provider, SearchProvider::DuckDuckGo));
    }

    #[test]
    #[serial_test::serial]
    fn test_search_provider_brave_missing_key() {
        unsafe {
            env::set_var("GIA_SEARCH_API", "brave");
            env::remove_var("GIA_BRAVE_API_KEY");
        }
        let result = SearchProvider::from_env();
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("GIA_BRAVE_API_KEY"));
    }

    #[test]
    #[serial_test::serial]
    fn test_search_provider_brave_with_key() {
        unsafe {
            env::set_var("GIA_SEARCH_API", "brave");
            env::set_var("GIA_BRAVE_API_KEY", "test_key");
        }
        let provider = SearchProvider::from_env().unwrap();
        assert!(matches!(provider, SearchProvider::Brave { .. }));
        unsafe {
            env::remove_var("GIA_SEARCH_API");
            env::remove_var("GIA_BRAVE_API_KEY");
        }
    }

    #[test]
    fn test_format_duckduckgo_results() {
        let response = DuckDuckGoResponse {
            abstract_text: "Rust is a programming language".to_string(),
            abstract_url: "https://rust-lang.org".to_string(),
            related_topics: vec![RelatedTopic::Topic {
                text: "Rust tutorial".to_string(),
                first_url: "https://example.com/tutorial".to_string(),
            }],
            heading: "Rust Programming".to_string(),
        };

        let formatted = format_duckduckgo_results(&response, "rust programming");
        assert!(formatted.contains("Rust Programming"));
        assert!(formatted.contains("Rust is a programming language"));
        assert!(formatted.contains("https://rust-lang.org"));
        assert!(formatted.contains("Rust tutorial"));
    }

    #[test]
    fn test_format_brave_results() {
        let response = BraveResponse {
            web: BraveWebResults {
                results: vec![
                    BraveResult {
                        title: "Rust Programming".to_string(),
                        description: "Learn Rust".to_string(),
                        url: "https://rust-lang.org".to_string(),
                    },
                    BraveResult {
                        title: "Rust Tutorial".to_string(),
                        description: "Step by step guide".to_string(),
                        url: "https://example.com/tutorial".to_string(),
                    },
                ],
            },
        };

        let formatted = format_brave_results(&response, "rust programming");
        assert!(formatted.contains("Rust Programming"));
        assert!(formatted.contains("Learn Rust"));
        assert!(formatted.contains("https://rust-lang.org"));
        assert!(formatted.contains("Rust Tutorial"));
    }

    #[tokio::test]
    async fn test_search_web_query_too_long() {
        let tool = SearchWebTool;
        let long_query = "a".repeat(501);
        let args = json!({ "query": long_query });
        let context = SecurityContext::new();

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Search query too long"));
    }

    #[tokio::test]
    async fn test_search_web_disabled() {
        let tool = SearchWebTool;
        let args = json!({ "query": "test" });
        let context = SecurityContext::new().with_allow_web_search(false);

        let result = tool.execute(args, &context).await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Web search is disabled"));
    }
}
