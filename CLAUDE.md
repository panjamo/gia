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

# Text-to-speech output
cargo run -- "Tell me a short story" --tts
cargo run -- "What is the weather today?" -T
cargo run -- "Erzähl mir eine Geschichte" --tts --tts-language de-DE
cargo run -- "Tell me a joke" -T -L en-US
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

### Key Design Patterns

**Multi-API Key Support**: The application supports pipe-separated API keys (`key1|key2|key3`) and automatically falls back to alternative keys when encountering 429 rate limit errors. This is handled in `gemini.rs` with cooperation from `api_key.rs`.

**Input Sources**: Four input sources can be combined:
1. Command line arguments (primary prompt)
2. Clipboard content (with `-c` flag) - supports both text and images
3. Stdin content (automatically detected when available)
4. Text file content (with `-f` flag) - can be used multiple times

**Output Destinations**: Four output options:
1. Stdout (default)
2. Clipboard (with `-o` flag)
3. Browser preview (with `-b` flag)
4. Text-to-speech (with `-T` or `--tts` flag)

**Conversation Management**: Persistent conversation storage allowing users to resume previous conversations:
- Local JSON-based storage in `~/.gia/conversations/`
- Automatic conversation history inclusion in prompts
- Context window management with automatic truncation
- Support for resuming latest or specific conversations

**Error Handling**: Comprehensive error handling with user-friendly messages for common issues like missing API keys, authentication failures, and rate limits.

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