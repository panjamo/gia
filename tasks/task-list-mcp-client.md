# Task List: MCP Client Integration (Issue #2)

## Phase 1: Basic MCP Client (MVP)

### Goal
Connect to one MCP server (file operations) and call tools manually.

### Tasks

- [x] **1. Research and Planning**
  - [x] 1.1 Research mcp-tools crate API documentation
  - [x] 1.2 Review mcp-tools examples and client usage patterns
  - [x] 1.3 Identify integration points in existing gia architecture

- [x] **2. Dependency Setup**
  - [x] 2.1 Add mcp-tools dependency to gia/Cargo.toml
  - [x] 2.2 Verify dependency builds correctly
  - [x] 2.3 Update Cargo.lock

- [x] **3. Core Module Creation**
  - [x] 3.1 Create gia/src/mcp_client.rs module file
  - [x] 3.2 Define McpClient struct and basic types
  - [x] 3.3 Define McpError enum for error handling
  - [x] 3.4 Add module declaration in main.rs

- [x] **4. Connection Implementation**
  - [x] 4.1 Implement stdio-based connection to MCP server
  - [x] 4.2 Add connection initialization logic
  - [x] 4.3 Add connection error handling
  - [x] 4.4 Add connection cleanup/disconnect logic

- [x] **5. Tool Discovery**
  - [x] 5.1 Implement list_tools() wrapper
  - [x] 5.2 Parse and structure tool metadata
  - [x] 5.3 Add caching for tool lists
  - [x] 5.4 Add error handling for discovery failures

- [x] **6. Tool Execution**
  - [x] 6.1 Implement call_tool() wrapper
  - [x] 6.2 Add JSON parameter serialization
  - [x] 6.3 Add timeout handling (default 30s)
  - [x] 6.4 Parse and return tool results
  - [x] 6.5 Add error handling for execution failures

- [x] **7. CLI Integration**
  - [x] 7.1 Add --mcp-server CLI argument to Args struct
  - [x] 7.2 Add argument validation
  - [x] 7.3 Update help text with MCP server usage
  - [ ] 7.4 Add --list-mcp-tools command (for Phase 2)

- [x] **8. Main Application Integration**
  - [x] 8.1 Initialize MCP client in main() when --mcp-server is provided
  - [x] 8.2 Connect to specified server on startup
  - [x] 8.3 Pass MCP client to prompt handling logic
  - [x] 8.4 Add graceful fallback if connection fails

- [x] **9. Error Handling & Logging**
  - [x] 9.1 Map MCP errors to user-friendly messages
  - [x] 9.2 Add logging for connection events
  - [x] 9.3 Add logging for tool discovery
  - [x] 9.4 Add logging for tool execution
  - [x] 9.5 Ensure errors go to stderr, not stdout

- [x] **10. Testing**
  - [x] 10.1 Create test file tests/mcp_client_test.rs
  - [x] 10.2 Write unit tests for McpClient struct
  - [x] 10.3 Write integration test for file server connection
  - [x] 10.4 Write test for tool discovery
  - [x] 10.5 Write test for tool execution
  - [x] 10.6 Add error case tests

- [x] **11. Documentation**
  - [x] 11.1 Add inline documentation to mcp_client.rs
  - [x] 11.2 Update README.md with MCP client usage
  - [x] 11.3 Update CLAUDE.md with MCP development notes
  - [x] 11.4 Add example commands to documentation

- [x] **12. Manual Testing & Validation**
  - [x] 12.1 Test connection to unified mcp-server
  - [x] 12.2 Test list_tools() with tool categories
  - [x] 12.3 Implement TCP connection (discovered unified architecture)
  - [x] 12.4 Update mock tools for all 4 categories (git, code, web, system)
  - [x] 12.5 Update documentation for unified server architecture

---

## Phase 2: Configuration & Multiple Servers (Future)

- [ ] Implement ~/.gia/mcp-config.json configuration file
- [ ] Add support for all 5 predefined servers
- [ ] Add multi-server support (comma-separated)
- [ ] Implement --list-mcp-tools command fully

## Phase 3: LLM Autonomous Tool Selection (Future)

- [ ] Enhance LLM prompt with tool descriptions
- [ ] Implement tool invocation detection in LLM responses
- [ ] Tool execution + result feedback loop
- [ ] Conversation history integration

## Phase 4: Advanced Features (Future)

- [ ] Multi-step tool chaining
- [ ] Tool result caching
- [ ] TCP connection support
- [ ] Performance optimization

---

**Status**: Ready to begin Phase 1, Task 1.1
**Last Updated**: 2025-10-17
