# Tool Use / Function Calling Implementation Plan for gia CLI

**GitHub Issue**: https://github.com/panjamo/gia/issues/1

## Design Principles: KISS & DRY

### Keep It Simple, Stupid (KISS)
- **Single Responsibility**: Each tool does ONE thing well (read file, write file, list dir, etc.)
- **Trait-based abstraction**: One `GiaTool` trait, simple to implement, easy to understand
- **Minimal state**: Tools are stateless, security context is immutable per execution
- **Clear flow**: Linear execution loop (request → tool call → execute → response)
- **No premature optimization**: Start simple, optimize only when needed

### Don't Repeat Yourself (DRY)
- **Tool trait**: Common interface eliminates duplicate schema/execution patterns
- **Security context**: Single validation logic used by all tools
- **Executor**: Centralized tool execution with consistent error handling
- **Registry pattern**: Tools register themselves, no manual dispatch tables
- **Shared conversion logic**: Single `to_genai_tool()` method for all tools

## Overview

Enable the LLM to execute Rust functions (read/write files, list directories, execute commands, search web) and incorporate results back into the conversation flow.

## Key Findings

- ✅ **genai v0.4** has complete tool calling support built-in
- ✅ Current architecture already supports `ChatRole::Tool` in message types
- ✅ Provider abstraction (`AiProvider` trait) is well-structured for this feature
- ✅ Conversation management system can naturally store tool calls/results

## Architectural Approach

### Tool Execution Loop Pattern

```
1. User sends prompt with tools available
2. LLM receives ChatRequest with tool definitions
3. LLM decides to call tool(s) → returns ToolCall in response
4. Application executes tool(s) → generates ToolResponse
5. Application sends ToolResponse back to LLM (loop back to step 2)
6. LLM processes tool results → returns final text response
7. Application outputs final response to user
```

### Security Model (Opt-in, Defense in Depth)

**Opt-in with path-based sandboxing**:
- Tools disabled by default, enabled via `--enable-tools` flag
- Path allowlisting: Only access specific directories
- User must specify `--tool-allow-cwd` or `--tool-allowed-dir <DIR>`
- Max file size: 10MB
- Max tool iterations: 10 (prevent infinite loops)
- Command execution requires additional `--allow-command-execution` flag

### Example Usage

```bash
# Enable tools with current directory access
gia --enable-tools --tool-allow-cwd "Read src/main.rs and create a summary in SUMMARY.md"

# Enable tools with specific directory
gia --enable-tools --tool-allowed-dir ./project "Analyze the code"

# Command execution (requires extra flag)
gia --enable-tools --allow-command-execution --tool-allow-cwd "Use gh to list open PRs"

# Safer: Require confirmation for each command
gia --enable-tools --allow-command-execution --confirm-commands --tool-allow-cwd "Run cargo test"

# Disable specific tools
gia --enable-tools --tool-disable write_file,execute_command "Read config.json"
```

## Implementation Breakdown

### Phase 1: Core Infrastructure

**New Files**:

1. **`gia/src/tools/mod.rs`** - Module entry point
   - Export public API: `ToolRegistry`, `ToolExecutor`, `SecurityContext`
   - Declare submodules: `registry`, `implementations`, `executor`, `security`

2. **`gia/src/tools/registry.rs`** - Tool definition system (DRY principle)
   ```rust
   // Single trait defines tool interface
   pub trait GiaTool: Send + Sync {
       fn name(&self) -> &str;
       fn description(&self) -> &str;
       fn schema(&self) -> Value;
       async fn execute(&self, args: Value, context: &SecurityContext) -> Result<String>;

       // DRY: Common conversion logic
       fn to_genai_tool(&self) -> Tool {
           Tool::new(self.name())
               .with_description(self.description())
               .with_schema(self.schema())
       }
   }

   // Registry pattern: Tools register themselves
   pub struct ToolRegistry {
       tools: HashMap<String, Box<dyn GiaTool>>,
   }
   ```

3. **`gia/src/tools/security.rs`** - Security sandboxing (DRY validation)
   ```rust
   pub struct SecurityContext {
       allowed_dirs: HashSet<PathBuf>,
       max_file_size: usize,
       allow_web_search: bool,
       allow_command_execution: bool,
       command_timeout: Duration,
       confirm_commands: bool,
   }

   impl SecurityContext {
       // DRY: Single validation method used by all file tools
       pub fn is_path_allowed(&self, path: &Path) -> bool { /* ... */ }

       // DRY: Single command validation used by ExecuteCommandTool
       pub fn is_command_allowed(&self, command: &str) -> bool { /* ... */ }
   }
   ```

4. **`gia/src/tools/executor.rs`** - Tool execution orchestration (DRY error handling)
   ```rust
   pub struct ToolExecutor {
       registry: ToolRegistry,
       security_context: SecurityContext,
   }

   impl ToolExecutor {
       // DRY: Centralized execution with consistent error handling
       pub async fn execute_tool_call(&self, tool_call: &ToolCall) -> ToolResponse {
           log_info(&format!("Executing tool: {}", tool_call.fn_name));

           let result = match self.registry.get(&tool_call.fn_name) {
               Some(tool) => tool.execute(tool_call.fn_arguments.clone(), &self.security_context).await,
               None => Err(anyhow!("Unknown tool: {}", tool_call.fn_name)),
           };

           ToolResponse::new(tool_call.call_id.clone(), result.unwrap_or_else(|e| format!("Error: {}", e)))
       }

       // Parallel execution for efficiency
       pub async fn execute_tool_calls(&self, calls: Vec<ToolCall>) -> Vec<ToolResponse> {
           futures::future::join_all(calls.iter().map(|c| self.execute_tool_call(c))).await
       }
   }
   ```

5. **`gia/src/tools/implementations.rs`** - Tool implementations (KISS principle)
   - `ReadFileTool`: Read file contents with path validation
   - `WriteFileTool`: Write/create files with directory creation
   - `ListDirectoryTool`: List directory contents with icons
   - `SearchWebTool`: Web search (placeholder for MVP)
   - `ExecuteCommandTool`: Execute shell commands with safeguards

   Each tool follows the same pattern:
   ```rust
   pub struct ReadFileTool;

   impl GiaTool for ReadFileTool {
       fn name(&self) -> &str { "read_file" }
       fn description(&self) -> &str { "Read the contents of a text file..." }
       fn schema(&self) -> Value { json!({ /* JSON schema */ }) }

       async fn execute(&self, args: Value, context: &SecurityContext) -> Result<String> {
           // 1. Parse args
           // 2. Validate with context.is_path_allowed()
           // 3. Execute operation
           // 4. Return result
       }
   }
   ```

### Phase 2: Integration with Existing Code

**Modified Files**:

1. **`gia/src/cli.rs`** - Add CLI flags
   - `--enable-tools`: Enable tool/function calling (default: false)
   - `--tool-allow-cwd`: Allow tools to access current working directory
   - `--tool-allowed-dir <DIR>`: Allow tools to access specific directory
   - `--tool-disable <TOOLS>`: Disable specific tools (comma-separated)
   - `--allow-command-execution`: Allow ExecuteCommandTool (requires --enable-tools)
   - `--command-timeout <SECS>`: Command execution timeout in seconds (default: 30)
   - `--confirm-commands`: Require user confirmation before each command execution

2. **`gia/src/app.rs`** - Main application flow (KISS: clear separation)
   ```rust
   pub async fn run_app(config: Config) -> Result<()> {
       // ... existing setup ...

       // Initialize tools if enabled (KISS: simple conditional)
       let tool_executor = if config.enable_tools {
           Some(initialize_tool_executor(&config)?)
       } else {
           None
       };

       // Tool execution loop (KISS: separate function)
       let (final_response, all_messages) = if let Some(executor) = &tool_executor {
           execute_with_tools(&mut provider, messages, user_message, executor, &config).await?
       } else {
           execute_without_tools(&mut provider, messages, user_message).await?
       };

       // ... rest of existing code ...
   }

   // KISS: Single responsibility - execute with tools
   async fn execute_with_tools(
       provider: &mut Box<dyn AiProvider>,
       mut messages: Vec<ChatMessage>,
       user_message: ChatMessageWrapper,
       executor: &ToolExecutor,
       config: &Config,
   ) -> Result<(String, Vec<ChatMessageWrapper>)> {
       const MAX_TOOL_ITERATIONS: usize = 10;

       messages.push(user_message.to_genai_chat_message()?);
       let mut conversation_wrappers = vec![user_message];

       // Simple loop until no more tool calls
       for iteration in 0..MAX_TOOL_ITERATIONS {
           let chat_req = ChatRequest::new(messages.clone())
               .with_tools(executor.registry.to_genai_tools());

           let response = provider.generate_content_with_chat_request(chat_req).await?;
           let tool_calls = response.tool_calls();

           if tool_calls.is_empty() {
               // No tool calls - return final response
               return Ok((response.first_text()?.to_string(), conversation_wrappers));
           }

           // Execute tools and add results to messages
           let tool_responses = executor.execute_tool_calls(tool_calls).await;
           for tr in &tool_responses {
               messages.push(ChatMessage::from(tr.clone()));
               conversation_wrappers.push(/* wrap tool response */);
           }
       }

       Err(anyhow!("Tool execution loop exceeded max iterations"))
   }

   // DRY: Centralized tool registration
   fn initialize_tool_executor(config: &Config) -> Result<ToolExecutor> {
       let mut registry = ToolRegistry::new();

       // Register tools (KISS: straightforward registration)
       registry.register(Box::new(ReadFileTool));
       registry.register(Box::new(WriteFileTool));
       registry.register(Box::new(ListDirectoryTool));
       registry.register(Box::new(SearchWebTool));

       if config.allow_command_execution {
           registry.register(Box::new(ExecuteCommandTool));
       }

       // Build security context from config
       let security = SecurityContext::new()
           .with_max_file_size(10 * 1024 * 1024)
           .with_allow_web_search(true)
           .with_allow_command_execution(config.allow_command_execution)
           .with_command_timeout(Duration::from_secs(config.command_timeout))
           .with_confirm_commands(config.confirm_commands);

       if config.tool_allow_cwd {
           security = security.allow_current_dir()?;
       }

       if let Some(ref dir) = config.tool_allowed_dir {
           security = security.with_allowed_dir(dir);
       }

       Ok(ToolExecutor::new(registry, security))
   }
   ```

3. **`gia/src/content_part_wrapper.rs`** - Message type extensions
   - Add to `ContentPartWrapper`:
     - `ToolCall { call_id, fn_name, fn_arguments }`
     - `ToolResult { call_id, content }`
   - Add to `MessageContentWrapper`:
     - `ToolCalls { tool_calls: Vec<ToolCallInfo> }`
     - `ToolResponse { call_id, content }`
   - Update `to_genai_content_part()` for new variants (DRY: single conversion method)

4. **`gia/src/conversation.rs`** - Conversation persistence
   - Add `ToolCall` and `ToolResult` to `ResourceType` enum
   - Update `format_as_chat_markdown()`: Display tool interactions with 🔧 icons
   - Update `truncate_if_needed()`: Keep complete tool interaction sequences

5. **`gia/Cargo.toml`** - Dependencies
   - Add `futures = "0.3"` for parallel tool execution

### Phase 3: Provider Integration

**No changes needed** to `gemini.rs` or `ollama.rs`:
- Both already work with `genai::chat::ChatMessage` which supports tool roles
- genai v0.4 handles tool serialization/deserialization automatically
- Tool calls extracted from `ChatResponse` via `.tool_calls()`

## Critical Files for Implementation

1. **`gia/src/tools/mod.rs`** (NEW) - Module entry point
2. **`gia/src/tools/registry.rs`** (NEW) - Core trait and registry
3. **`gia/src/tools/security.rs`** (NEW) - Security context and validation
4. **`gia/src/tools/executor.rs`** (NEW) - Execution orchestration
5. **`gia/src/tools/implementations.rs`** (NEW) - All tool implementations
6. **`gia/src/app.rs`** (MODIFY) - Main tool execution loop
7. **`gia/src/cli.rs`** (MODIFY) - CLI flags
8. **`gia/src/content_part_wrapper.rs`** (MODIFY) - Message type support
9. **`gia/src/conversation.rs`** (MODIFY) - Conversation persistence

## ExecuteCommandTool Implementation Details

### Security Safeguards (Defense in Depth)

1. **Blocklist of Dangerous Commands**:
   ```rust
   const BLOCKED_COMMANDS: &[&str] = &[
       "rm -rf", "rm-rf", "rmdir /s",
       "dd", "mkfs", "format",
       ":(){ :|:& };:", // Fork bomb
       "chmod -R 777", "chown -R",
       "iptables", "ufw disable",
   ];
   ```

2. **Command Validation** (KISS: simple checks):
   - Check command against blocklist (case-insensitive)
   - Validate working directory is within allowed paths
   - Reject commands with suspicious patterns

3. **Execution Model**:
   ```rust
   // KISS: Straightforward command execution with timeout
   let output = Command::new(shell)
       .args(&["-c", command])
       .current_dir(working_dir)
       .env_clear()  // Security: Clear environment
       .env("PATH", std::env::var("PATH")?)  // Restore PATH only
       .stdout(Stdio::piped())
       .stderr(Stdio::piped())
       .kill_on_drop(true)
       .spawn()?
       .wait_with_output()
       .await
       .timeout(Duration::from_secs(timeout))?;
   ```

4. **User Confirmation Flow** (when `--confirm-commands` is enabled):
   ```
   🔧 AI wants to execute command:

   Command: gh pr create --title "Fix bug" --body "..."
   Working directory: /home/user/project
   Timeout: 30s

   Allow this command? [y/N]
   ```

### Cross-Platform Shell Detection

```rust
// KISS: Simple platform-specific shell selection
fn get_default_shell() -> (&'static str, &'static str) {
    #[cfg(target_os = "windows")]
    return ("cmd", "/C");

    #[cfg(not(target_os = "windows"))]
    {
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
```

## Testing Strategy

### Unit Tests (KISS: Simple, focused tests)

**File**: `gia/src/tools/implementations.rs`
- `test_read_file_success()`: Read file from allowed directory
- `test_read_file_denied()`: Reject file outside allowed directories
- `test_write_file_success()`: Write file with directory creation
- `test_list_directory_success()`: List directory contents
- `test_security_context_validation()`: Path validation logic
- `test_command_blocklist()`: Dangerous commands are blocked

### Integration Tests

**File**: `gia/tests/tool_integration_test.rs`
- `test_tool_execution_loop()`: Full flow with mock provider
- `test_conversation_with_tools()`: Tool interactions saved correctly
- `test_conversation_resumption()`: Resume conversation with tool history

### Manual Testing Scenarios

1. File reading: `gia --enable-tools --tool-allow-cwd "Read config.json"`
2. File writing: `gia --enable-tools --tool-allow-cwd "Create TODO.md with 5 tasks"`
3. Directory exploration: `gia --enable-tools --tool-allow-cwd "What files are in src/"`
4. Multiple tools: `gia --enable-tools --tool-allow-cwd "List files, read main.rs, create summary.md"`
5. Command execution: `gia --enable-tools --allow-command-execution --tool-allow-cwd "Use gh to list open PRs"`
6. Command with confirmation: `gia --enable-tools --allow-command-execution --confirm-commands --tool-allow-cwd "Run cargo test"`
7. Security: `gia --enable-tools --tool-allow-cwd "Read /etc/passwd"` (should be denied)
8. Dangerous command blocked: `gia --enable-tools --allow-command-execution --tool-allow-cwd "Delete everything with rm -rf"` (should be blocked)
9. Error handling: `gia --enable-tools --tool-allow-cwd "Read nonexistent.txt"` (graceful failure)

## Security Considerations

1. **Path Traversal Prevention**: Canonicalize paths before validation
2. **Symlink Attacks**: `canonicalize()` resolves all symlinks
3. **Resource Limits**: Max file size (10MB), max iterations (10)
4. **Information Disclosure**: Sanitize error messages
5. **Command Execution Security** (for ExecuteCommandTool):
   - **Requires explicit flag**: `--allow-command-execution` in addition to `--enable-tools`
   - **Working directory restriction**: Commands run in allowed directories only
   - **Command timeout**: Default 30 seconds, configurable via `--command-timeout`
   - **No shell injection**: Use proper argument passing (not shell string parsing)
   - **Dangerous commands blocked**: Maintain blocklist (rm -rf, dd, mkfs, etc.)
   - **User confirmation**: Optional `--confirm-commands` flag for interactive approval
6. **User Awareness**: Display warning when tools are enabled

### ExecuteCommandTool Security Model

```
⚠️  DANGER: Command execution enabled!
   The AI can run shell commands (bash, cmd, powershell, zsh)
   Working directory: /home/user/project
   Timeout: 30 seconds
   Blocked commands: rm -rf, dd, mkfs, format

   Use --confirm-commands to approve each command before execution
```

## Implementation Phases

### Phase 1: Core Infrastructure (Priority 1)
- [ ] Create tools module structure (`mod.rs`, `registry.rs`, `security.rs`, `executor.rs`)
- [ ] Implement `GiaTool` trait and `ToolRegistry`
- [ ] Implement `SecurityContext` with path validation
- [ ] Implement `ToolExecutor` with centralized error handling
- [ ] Add CLI flags to `cli.rs`
- [ ] Write unit tests for security validation

### Phase 2: Basic Tools (Priority 1)
- [ ] Implement `ReadFileTool`
- [ ] Implement `WriteFileTool`
- [ ] Implement `ListDirectoryTool`
- [ ] Implement `SearchWebTool` (placeholder)
- [ ] Implement `ExecuteCommandTool` with security safeguards
- [ ] Write unit tests for each tool

### Phase 3: Execution Loop (Priority 1)
- [ ] Add `execute_with_tools()` to `app.rs`
- [ ] Add `initialize_tool_executor()` to `app.rs`
- [ ] Integrate with provider flow in `run_app()`
- [ ] Add tool execution logging

### Phase 4: Conversation Integration (Priority 2)
- [ ] Extend `ContentPartWrapper` for tool calls/results
- [ ] Extend `MessageContentWrapper` for tool calls/responses
- [ ] Update conversation serialization
- [ ] Update `format_as_chat_markdown()` for tool display

### Phase 5: Polish & Testing (Priority 2)
- [ ] Write integration tests
- [ ] Manual testing scenarios
- [ ] Documentation updates (CLAUDE.md, README.md)
- [ ] Security audit

### Phase 6: Advanced Features (Future)
- [ ] Custom tool plugins
- [ ] Web search API integration (DuckDuckGo, Brave)
- [ ] Streaming tool execution output
- [ ] Tool usage analytics and logging
- [ ] Additional specialized tools (git operations wrapper, HTTP request tool)

## Verification

After implementation, verify:

1. **Basic functionality**:
   ```bash
   gia --enable-tools --tool-allow-cwd "Read README.md and summarize it"
   ```

2. **Security**:
   ```bash
   gia --enable-tools --tool-allow-cwd "Read /etc/passwd"  # Should be denied
   ```

3. **Conversation persistence**:
   ```bash
   gia --enable-tools --tool-allow-cwd "List files in src/"
   gia --resume "What files did you see?"  # Should remember tool results
   ```

4. **Error handling**:
   ```bash
   gia --enable-tools --tool-allow-cwd "Read nonexistent.txt"  # Should fail gracefully
   ```

5. **Multiple tools in sequence**:
   ```bash
   gia --enable-tools --tool-allow-cwd "Read src/main.rs and create a summary file"
   ```

6. **Command execution**:
   ```bash
   gia --enable-tools --allow-command-execution --tool-allow-cwd "Use gh to list open PRs"
   ```

## Dependencies

- `genai = "0.4"` ✅ Already included (has tool support)
- `futures = "0.3"` ⚠️ Need to add (for parallel tool execution)
- All other dependencies already present

## Notes

- genai v0.4 provides: `Tool`, `ToolCall`, `ToolResponse`, `ChatRole::Tool`
- Ollama support depends on model capabilities (some models support tools, others don't)
- Start with file tools, add web search later with proper API integration
- Consider adding environment variable `GIA_ENABLE_TOOLS=1` as alternative to CLI flag

## KISS & DRY Checklist

### KISS (Keep It Simple)
- ✅ Single trait for all tools
- ✅ Stateless tool design
- ✅ Linear execution flow
- ✅ Clear separation of concerns (registry, executor, security)
- ✅ Simple error handling (Result<String, Error>)
- ✅ No complex state machines or async orchestration

### DRY (Don't Repeat Yourself)
- ✅ Common tool interface (`GiaTool` trait)
- ✅ Shared security validation (`SecurityContext`)
- ✅ Centralized execution logic (`ToolExecutor`)
- ✅ Single conversion method (`to_genai_tool()`)
- ✅ Unified error handling pattern
- ✅ Reusable path validation logic
