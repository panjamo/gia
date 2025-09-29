# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

GIA (Google Intelligence Assistant) is a command-line tool that sends prompts to Google's Gemini API and returns AI responses. It supports multiple input sources (command line, clipboard, stdin, images) and output destinations (stdout, clipboard). The tool now supports multimodal interactions with images (JPEG, PNG, WebP, HEIC, PDF).

## Development Commands

### Build and Test
```bash
cargo build --release      # Production build
cargo build                # Development build  
cargo test                 # Run tests
cargo test -- --nocapture  # Run tests with output
```

### Code Quality
```bash
cargo clippy --fix --allow-dirty  # Fix linting issues
cargo fmt                         # Format code
cargo check                      # Check compilation without building
```

### Running
```bash
# Development
cargo run -- "your prompt here"

# After building
./target/release/gia "your prompt here"
./target/debug/gia "your prompt here"

# Resume conversations
cargo run -- --resume "continue previous conversation"
cargo run -- --resume abc123 "continue specific conversation"
cargo run -- --list-conversations  # List all saved conversations

# Image analysis
cargo run -- "What do you see in this image?" -i photo.jpg
cargo run -- "Compare these images" -i img1.jpg -i img2.png

# Text file input
cargo run -- "Summarize these documents" -f document1.txt -f document2.txt
cargo run -- "What are the differences between these files?" -f old.txt -f new.txt

# Combining multiple input sources
cargo run -- "Analyze this code and documentation" -f README.md -f main.rs -i diagram.png

# Clipboard image analysis (copy an image to clipboard first)
cargo run -- "What do you see in this image?" -c

# MCP server integration

## Loki MCP Server Examples

### Stdio Transport (Process-based)
# Connect to Loki MCP server and list available tools
cargo run -- --mcp-server "loki:C:\bin\loki-mcp-server.exe" --list-mcp-tools

### HTTP Transport (Network-based)  
# Connect to Loki MCP server via HTTP and list available tools
cargo run -- --mcp-server "loki-http:http://127.0.0.1:8080" --list-mcp-tools
cargo run -- --mcp-server "loki-remote:https://loki.example.com/mcp" --list-mcp-tools

### Query Examples (work with both transports)
# Query Loki logs (basic query) - Stdio transport
cargo run -- --mcp-server "loki:C:\bin\loki-mcp-server.exe" --mcp-call loki:loki_query:'{"query":"{app=\"myapp\"}", "limit": 100}'

# Query Loki logs - HTTP transport
cargo run -- --mcp-server "loki-http:http://127.0.0.1:8080" --mcp-call loki:loki_query:'{"query":"{app=\"myapp\"}", "limit": 100}'

# Query logs with environment filter
cargo run -- --mcp-server "loki:C:\bin\loki-mcp-server.exe" --mcp-call loki:loki_query:'{"query":"{environment=\"tst\"} |= \"error\"", "limit": 50}'

# Get all available label names
cargo run -- --mcp-server "loki:C:\bin\loki-mcp-server.exe" --mcp-call loki:loki_label_names:'{}'

# Get values for a specific label
cargo run -- --mcp-server "loki:C:\bin\loki-mcp-server.exe" --mcp-call loki:loki_label_values:'{"label": "app"}'

# Query with time range
cargo run -- --mcp-server "loki:C:\bin\loki-mcp-server.exe" --mcp-call loki:loki_query:'{"query":"{app=\"ezp-connection-service\"}", "start": "2024-01-01T00:00:00Z", "end": "2024-01-02T00:00:00Z", "limit": 100}'

# Complex LogQL query for print job tracking
cargo run -- --mcp-server "loki:C:\bin\loki-mcp-server.exe" --mcp-call loki:loki_query:'{"query":"{environment=\"tst\"} |~ \"EZP_TRACE_ID.*printjob-id\" | json | line_format \"{{.EZP_TRACE_ID}} -> {{.printJobId}}\"", "limit": 50}'

### Mixed Transport Usage
# Use both stdio and HTTP servers simultaneously
cargo run -- --mcp-server "local:C:\bin\loki-mcp-server.exe" --mcp-server "remote:http://127.0.0.1:8080" --list-mcp-tools

## AI-Assisted Log Analysis Examples
# Analyze errors and get AI suggestions
cargo run -- --mcp-server loki:"C:\bin\loki-mcp-server.exe" --mcp-call loki:loki_query:'{"query":"{app=\"ezp-connection-service\"} |= \"error\"", "limit": 20}' "Analyze these error logs and suggest troubleshooting steps"

# Print job failure analysis
cargo run -- --mcp-server loki:"C:\bin\loki-mcp-server.exe" --mcp-call loki:loki_query:'{"query":"{module=\"EFACon.exe\"} |= \"failed\"", "limit": 10}' "What are the common patterns in these print job failures?"

# Performance analysis
cargo run -- --mcp-server loki:"C:\bin\loki-mcp-server.exe" --mcp-call loki:loki_query:'{"query":"{app=\"ezp-rendering-backend\"} |~ \"duration.*ms\"", "limit": 50}' "Analyze the performance patterns and identify potential bottlenecks"

## Other MCP Server Examples
cargo run -- --mcp-server filesystem:node:server.js --list-mcp-tools
cargo run -- --mcp-server myserver:python:mcp_server.py --mcp-call myserver:read_file:'{"path":"README.md"}'
```

### Environment Setup
Set API key(s):
```bash
# Single key
export GEMINI_API_KEY="your_api_key_here"

# Multiple keys for fallback (pipe-separated)
export GEMINI_API_KEY="key1|key2|key3"
```

Windows:
```cmd
set GEMINI_API_KEY=your_api_key_here
set GEMINI_API_KEY=key1|key2|key3
```

### Logging
```bash
RUST_LOG=debug cargo run -- "test"  # Debug logging
RUST_LOG=info cargo run -- "test"   # Info logging
RUST_LOG=error cargo run -- "test"  # Error logging only
```

## Architecture

### Module Structure
- `main.rs` - CLI argument parsing, main application flow
- `gemini.rs` - Gemini API client with rate limit fallback logic  
- `api_key.rs` - API key management (multiple keys, validation, fallback)
- `clipboard.rs` - Clipboard operations using arboard
- `conversation.rs` - Conversation management with persistent storage
- `logging.rs` - Structured logging to stderr
- `mcp.rs` - Model Context Protocol client implementation

### Key Design Patterns

**Multi-API Key Support**: The application supports pipe-separated API keys (`key1|key2|key3`) and automatically falls back to alternative keys when encountering 429 rate limit errors. This is handled in `gemini.rs` with cooperation from `api_key.rs`.

**Input Sources**: Four input sources can be combined:
1. Command line arguments (primary prompt)
2. Clipboard content (with `-c` flag) - supports both text and images
3. Stdin content (automatically detected when available)
4. Text file content (with `-f` flag) - can be used multiple times

**Output Destinations**: Two output options:
1. Stdout (default)
2. Clipboard (with `-o` flag)

**Conversation Management**: Persistent conversation storage allowing users to resume previous conversations:
- Local JSON-based storage in `~/.gia/conversations/`
- Automatic conversation history inclusion in prompts
- Context window management with automatic truncation
- Support for resuming latest or specific conversations

**Error Handling**: Comprehensive error handling with user-friendly messages for common issues like missing API keys, authentication failures, and rate limits.

**MCP Integration**: Support for Model Context Protocol (MCP) servers, allowing integration with external tools and resources:
- Multiple transport protocols: stdio (process-based) and HTTP (network-based)
- Connect to local MCP servers via stdin/stdout or remote servers via HTTP/HTTPS
- List available tools from connected servers
- Execute tool calls with JSON arguments
- Support for mixed transport scenarios (multiple servers with different transports)
- Seamless integration with existing conversation flows

**Loki MCP Integration**: Specialized support for Loki log analysis:
- Query logs using LogQL syntax via both stdio and HTTP transports
- Print job tracking with EZP_TRACE_ID correlation
- Environment-based filtering (tst, prod, etc.)
- Label management and discovery
- Time-range queries for incident analysis
- Support for cloud-hosted and local Loki MCP instances

### API Key Management
The `api_key.rs` module handles:
- Loading keys from `GEMINI_API_KEY` environment variable
- Supporting both single keys and pipe-separated multiple keys
- Validation of Google API key format (39 chars, starts with "AIza")
- Random key selection for initial requests
- Alternative key selection for fallback scenarios
- User guidance when keys are missing or invalid

### Conversation Management
The `conversation.rs` module handles:
- Persistent conversation storage in JSON format
- Automatic conversation history management
- Context window optimization through intelligent truncation
- UUID-based conversation identification
- Conversation listing and resumption functionality

### Testing
Tests use the `serial_test` crate to prevent environment variable conflicts when running in parallel. Tests cover:
- API key parsing and validation
- Multi-key fallback logic
- Input/output handling
- Conversation creation, serialization, and history management

## Important Notes

- All logging goes to stderr, leaving stdout clean for piping
- The tool validates API key format but continues with warnings if invalid
- Rate limit handling automatically tries alternative keys if available
- Windows-specific registry support was removed in favor of environment variables only
- Clipboard input automatically detects and handles both text and images
- When using `-c` flag, if an image is in the clipboard, it will be treated as an image input rather than text