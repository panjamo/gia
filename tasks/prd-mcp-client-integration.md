# PRD: MCP Client Integration for gia

## Introduction

This document outlines the requirements for integrating MCP (Model Context Protocol) client capabilities into `gia`, enabling it to connect to and utilize external MCP tool servers. This integration will allow gia to extend its functionality beyond direct LLM interactions by leveraging specialized tools for file operations, git analysis, code analysis, web scraping, and system information gathering.

The implementation will use the [`mcp-tools`](https://crates.io/crates/mcp-tools) crate, which provides both the client library and five predefined MCP servers.

## Goals

1. **Enable Tool Extension**: Allow gia to use external MCP tools to perform tasks beyond pure LLM capabilities
2. **Leverage Predefined Servers**: Integrate with the 5 predefined MCP servers from `mcp-tools`:
   - File Operations Server (filesystem access with security)
   - Git Tools Server (repository management and analysis)
   - Code Analysis Server (language-aware code analysis and refactoring)
   - Web Tools Server (web scraping and HTTP operations)
   - System Tools Server (system information and process management)
3. **Maintain User Experience**: Keep gia's simple, clean interface while adding powerful tool capabilities
4. **Support Incremental Adoption**: Allow users to opt-in to MCP features gradually

## User Stories

### US-1: File Analysis
**As a** developer  
**I want to** ask gia to analyze files in my project using natural language  
**So that** I can understand code structure without manually reading every file  

**Example**: `gia "Analyze the error handling in src/main.rs" --mcp-server file`

### US-2: Git Repository Insights
**As a** developer  
**I want to** ask gia questions about my git repository history  
**So that** I can quickly understand project evolution and contributions  

**Example**: `gia "What were the major changes in the last 5 commits?" --mcp-server git`

### US-3: Code Refactoring Suggestions
**As a** developer  
**I want to** get code analysis and refactoring suggestions through gia  
**So that** I can improve code quality efficiently  

**Example**: `gia "Suggest refactoring improvements for src/provider.rs" --mcp-server code`

### US-4: Web Research
**As a** user  
**I want to** ask gia to fetch and summarize web content  
**So that** I can research topics without leaving the command line  

**Example**: `gia "Fetch and summarize the latest Rust release notes" --mcp-server web`

### US-5: System Monitoring
**As a** system administrator  
**I want to** query system information through gia  
**So that** I can monitor system health in natural language  

**Example**: `gia "What processes are using the most memory?" --mcp-server system`

### US-6: Multi-Tool Workflows
**As a** power user  
**I want to** chain multiple MCP tool operations in a single query  
**So that** I can perform complex analysis tasks efficiently  

**Example**: `gia "Compare the code complexity between the current branch and main" --mcp-server git,code`

## Functional Requirements

### FR-1: MCP Client Connection

**FR-1.1**: gia MUST be able to connect to MCP servers using the `mcp-tools` crate's client API

**FR-1.2**: gia MUST support stdio-based connections (subprocess communication)

**FR-1.3**: gia SHOULD support TCP-based connections for remote MCP servers

**FR-1.4**: Connection failures MUST result in clear error messages indicating which server failed and why

**FR-1.5**: gia MUST gracefully fall back to normal operation if MCP server connection fails (with warning)

### FR-2: Server Configuration

**FR-2.1**: Users MUST be able to specify which MCP server(s) to use via CLI flag `--mcp-server <name>`

**FR-2.2**: gia MUST support a configuration file at `~/.gia/mcp-config.json` defining available MCP servers

**FR-2.3**: Configuration MUST include:
- Server name (e.g., "file", "git", "code", "web", "system")
- Connection type (stdio or TCP)
- Server binary path or TCP address
- Optional: Additional arguments/parameters

**FR-2.4**: gia MUST validate configuration on startup and warn about invalid entries

**FR-2.5**: Users SHOULD be able to specify multiple servers simultaneously (comma-separated)

**Example config structure**:
```json
{
  "servers": {
    "file": {
      "type": "stdio",
      "command": "mcp-file-server",
      "args": ["--secure"]
    },
    "git": {
      "type": "stdio",
      "command": "mcp-git-server"
    },
    "code": {
      "type": "tcp",
      "address": "localhost:3001"
    },
    "web": {
      "type": "stdio",
      "command": "mcp-web-server"
    },
    "system": {
      "type": "stdio",
      "command": "mcp-system-server"
    }
  }
}
```

### FR-3: Tool Discovery

**FR-3.1**: When connected to an MCP server, gia MUST discover available tools using the server's `list_tools()` API

**FR-3.2**: Tool discovery MUST happen automatically on connection

**FR-3.3**: gia MUST cache tool lists for the duration of the session to avoid repeated discovery calls

**FR-3.4**: gia MUST provide a CLI command to list available tools: `gia --list-mcp-tools`

**FR-3.5**: Tool listings MUST include:
- Tool name
- Tool description
- Required parameters
- Optional parameters
- Server providing the tool

### FR-4: LLM Integration for Tool Selection

**FR-4.1**: When MCP servers are configured, gia MUST include available tool information in the LLM context

**FR-4.2**: The LLM context MUST include:
- List of available tool names and descriptions
- Instructions on when and how to use each tool
- JSON schema for tool parameters

**FR-4.3**: gia MUST parse LLM responses to detect tool invocation requests

**FR-4.4**: Tool invocation syntax in LLM responses SHOULD follow this format:
```json
{
  "action": "use_tool",
  "tool": "tool_name",
  "parameters": {
    "param1": "value1",
    "param2": "value2"
  }
}
```

**FR-4.5**: gia MUST execute detected tool calls and feed results back to the LLM for further reasoning

**FR-4.6**: Tool execution results MUST be clearly indicated in the output (e.g., "🔧 Tool: read_file → Success")

### FR-5: Tool Execution

**FR-5.1**: gia MUST call MCP tools using the `call_tool()` API with proper JSON parameters

**FR-5.2**: Tool execution MUST have a configurable timeout (default: 30 seconds)

**FR-5.3**: Tool execution errors MUST be captured and reported to both user and LLM

**FR-5.4**: Tool execution MUST support streaming responses where applicable

**FR-5.5**: gia MUST log tool executions when `GIA_LOG_TO_FILE=1` is set, including:
- Tool name
- Parameters
- Execution time
- Result summary

### FR-6: Conversation Context Integration

**FR-6.1**: Tool execution results MUST be included in conversation history for `--resume` functionality

**FR-6.2**: Conversation storage MUST record:
- Which tools were used
- Tool parameters
- Tool results (truncated if large)

**FR-6.3**: When resuming conversations, gia MUST restore MCP server connections if they were used

**FR-6.4**: Tool results MUST count toward context window limits and be subject to truncation rules

### FR-7: Error Handling

**FR-7.1**: Connection errors MUST show: "❌ Failed to connect to MCP server '<name>': <reason>"

**FR-7.2**: Tool execution errors MUST show: "⚠️  Tool '<tool_name>' failed: <error_message>"

**FR-7.3**: Configuration errors MUST be reported on startup with suggestions for fixes

**FR-7.4**: If all configured MCP servers fail, gia MUST continue in normal mode with a warning

**FR-7.5**: Network timeout errors MUST suggest checking server status or increasing timeout

### FR-8: Output Formatting

**FR-8.1**: Tool invocations MUST be visually distinguished in output (e.g., with icons or color)

**FR-8.2**: Tool results MUST be formatted appropriately:
- JSON: Pretty-printed with syntax highlighting
- Text: Preserved formatting
- Binary: Indicated with summary (not raw data)

**FR-8.3**: Long tool results (>1000 lines) MUST be truncated with indication: "... (showing first 1000 lines)"

**FR-8.4**: When multiple tools are called, output MUST clearly show the sequence and results

## Non-Goals (Out of Scope)

1. **Creating MCP Servers**: gia will NOT implement its own MCP server capabilities (only client)
2. **MCP Server Development**: gia will NOT provide tools for building custom MCP servers
3. **Server Discovery**: gia will NOT automatically discover MCP servers on the network
4. **GUI Integration**: giagui will NOT support MCP features in Phase 1
5. **Complex Tool Orchestration**: Advanced workflow engines or DAG-based tool chaining
6. **Tool Marketplace**: No registry or marketplace for discovering third-party MCP servers
7. **Authentication**: No built-in auth/authz for MCP servers (relies on server-side security)
8. **Tool Versioning**: No management of tool version compatibility

## Incremental Implementation Phases

### Phase 1: Basic MCP Client (MVP)
**Goal**: Connect to one MCP server and call tools manually

**Deliverables**:
- [ ] Add `mcp-tools` dependency to `gia/Cargo.toml`
- [ ] Create `gia/src/mcp_client.rs` module
- [ ] Implement stdio connection to file operations server
- [ ] Implement `list_tools()` functionality
- [ ] Implement `call_tool()` with hardcoded parameters for testing
- [ ] Add CLI flag `--mcp-server <name>`
- [ ] Basic error handling and logging
- [ ] Update tests to cover MCP client basics

**Success Criteria**:
- User can run: `gia "test" --mcp-server file` and gia successfully connects to the file server
- User can manually invoke a file read operation through gia

### Phase 2: Configuration & Multiple Servers
**Goal**: Support all 5 predefined servers via configuration

**Deliverables**:
- [ ] Implement `~/.gia/mcp-config.json` configuration file support
- [ ] Configuration parser and validator
- [ ] Support for all 5 predefined servers (file, git, code, web, system)
- [ ] Multi-server support (comma-separated `--mcp-server`)
- [ ] CLI command `--list-mcp-tools`
- [ ] Improved error messages with configuration hints
- [ ] Documentation for setting up MCP servers

**Success Criteria**:
- User can configure multiple MCP servers in config file
- User can query available tools across all configured servers
- Configuration errors are clear and actionable

### Phase 3: LLM Autonomous Tool Selection
**Goal**: Let the LLM automatically decide which tools to use

**Deliverables**:
- [ ] Enhance LLM prompt with tool descriptions and schemas
- [ ] Implement tool invocation detection in LLM responses
- [ ] Tool execution + result feedback loop
- [ ] Context management for tool results
- [ ] Visual indicators for tool usage in output
- [ ] Conversation history integration

**Success Criteria**:
- User can ask: `gia "What files are in the current directory?" --mcp-server file` and gia automatically uses the list_files tool
- Tool results are fed back to LLM for final response formatting
- Conversation can be resumed with tool context intact

### Phase 4: Advanced Features
**Goal**: Tool chaining, optimization, and polish

**Deliverables**:
- [ ] Multi-step tool chaining (LLM can call multiple tools in sequence)
- [ ] Tool result caching within a session
- [ ] TCP connection support for remote servers
- [ ] Streaming tool results
- [ ] Enhanced logging and debugging
- [ ] Performance optimization (parallel tool calls where possible)
- [ ] Comprehensive integration tests

**Success Criteria**:
- User can ask complex queries that require multiple tool calls
- Tool executions are efficient and properly logged
- System handles edge cases gracefully

## Technical Considerations

### Integration Points

1. **Provider Architecture** (`gia/src/provider.rs`):
   - MCP client logic should be abstracted similarly to Gemini/Ollama providers
   - Consider creating an `McpToolProvider` trait

2. **Conversation Management** (`gia/src/conversation.rs`):
   - Extend `Conversation` struct to store tool execution metadata
   - Add serialization for MCP tool calls and results

3. **Logging** (`gia/src/logging.rs`):
   - Add MCP-specific log events
   - Ensure tool executions are traced properly

4. **Error Handling**:
   - Create `McpError` type in `mcp_client.rs`
   - Map MCP errors to user-friendly messages

### Dependencies

- **mcp-tools** (^0.1): Core MCP client and server implementations
- **tokio** (existing): Required for async MCP operations
- **serde_json** (existing): For tool parameter serialization

### Configuration Storage

- Location: `~/.gia/mcp-config.json`
- Format: JSON
- Validation: On startup, warn about invalid entries but continue
- Default: Empty (no MCP servers configured by default)

### Performance Considerations

1. **Connection Pooling**: Reuse MCP connections within a session
2. **Tool Discovery Caching**: Cache tool lists to avoid repeated calls
3. **Timeout Management**: Configurable timeouts to prevent hanging
4. **Result Truncation**: Limit tool result size to prevent context explosion

## Open Questions

1. **Q**: Should we bundle the 5 MCP server binaries with gia releases?
   - **A**: TBD - depends on size and complexity

2. **Q**: Should tool selection be purely automatic or allow manual override?
   - **A**: Both - automatic by default, with escape hatch for explicit tool calling

3. **Q**: How do we handle tool results that are too large for LLM context?
   - **A**: Truncate with summary, possibly implement semantic chunking in Phase 4

4. **Q**: Should we support custom/third-party MCP servers?
   - **A**: Yes, via configuration file (users provide binary path)

5. **Q**: What happens if the LLM hallucinates a non-existent tool?
   - **A**: Return error to LLM and let it retry with correct tool name

## Acceptance Criteria

### Phase 1 (MVP)
- [ ] gia can connect to the file operations MCP server
- [ ] gia can list available tools from the server
- [ ] gia can execute at least one file operation tool successfully
- [ ] Error messages are clear when server is unavailable
- [ ] Basic integration tests pass

### Phase 2 (Configuration)
- [ ] All 5 predefined servers can be configured
- [ ] Configuration validation works correctly
- [ ] `--list-mcp-tools` shows all tools from all configured servers
- [ ] Multi-server mode works with comma-separated values

### Phase 3 (LLM Integration)
- [ ] LLM can autonomously select and use appropriate tools
- [ ] Tool results are incorporated into LLM responses naturally
- [ ] Conversation history preserves tool execution context
- [ ] User can resume conversations with tool state intact

### Phase 4 (Advanced)
- [ ] Complex multi-tool workflows execute successfully
- [ ] Performance is acceptable (no noticeable lag)
- [ ] All edge cases have proper error handling
- [ ] Documentation is complete and accurate

---

**Document Version**: 1.0  
**Last Updated**: 2025-10-17  
**Status**: Draft - Ready for Review
