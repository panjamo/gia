# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

This is a Cargo workspace containing two binaries:

1. **gia** - Command-line tool that sends prompts to Google's Gemini API and returns AI responses. Supports multiple input sources (command line, clipboard, stdin, files) and output destinations (stdout, clipboard). Supports multimodal interactions with automatic detection of media files (JPEG, PNG, WebP, HEIC, PDF, MP3, MP4, etc.). Also supports local Ollama models.

2. **giagui** - GUI wrapper for gia using the egui framework. Must have gia installed and available in PATH.

## Development Commands

### Build and Test
```bash
# Build both binaries
cargo build --release      # Production build
cargo build                # Development build

# Build specific binaries
cargo build --release -p gia      # CLI only
cargo build --release -p giagui   # GUI only

# Test
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
# CLI Development
cargo run -p gia -- "your prompt here"

# GUI Development
cargo run -p giagui                    # Normal GUI mode
cargo run -p giagui -- --spinner       # Spinner-only mode (runs until killed)
cargo run -p giagui -- -s              # Spinner-only mode (short flag)

# After building
./target/release/gia "your prompt here"
./target/release/giagui                # Normal GUI
./target/release/giagui --spinner      # Spinner-only mode

# Using Ollama (local, requires Ollama running on localhost:11434)
cargo run -- -m "ollama::llama3.2" "your prompt here"

# Resume conversations
cargo run -- --resume "continue previous conversation"
cargo run -- --resume abc123 "continue specific conversation"
cargo run -- --list-conversations  # List all saved conversations

# Image analysis (auto-detected as media files)
cargo run -- "What do you see in this image?" -f photo.jpg
cargo run -- "Compare these images" -f img1.jpg -f img2.png

# Text file input
cargo run -- "Summarize these documents" -f document1.txt -f document2.txt
cargo run -- "What are the differences between these files?" -f old.txt -f new.txt

# Combining multiple input sources (auto-detection of media vs text)
cargo run -- "Analyze this code and documentation" -f README.md -f main.rs -f diagram.png

# Clipboard image analysis (copy an image to clipboard first)
cargo run -- "What do you see in this image?" -c

# Text-to-speech output
cargo run -- "Tell me a short story" --tts en-US
cargo run -- "What is the weather today?" -T en-US
cargo run -- "Erzähl mir eine Geschichte" --tts de-DE
cargo run -- "Erzähl mir eine Geschichte" --tts          # Uses default: de-DE
cargo run -- "Tell me a joke" -T                         # Uses default: de-DE

# Transcribe-only mode (no conversation history saved)
cargo run -- --record-audio --role EN --no-save         # English transcription only
cargo run -- --record-audio --role DE --no-save         # German transcription only
cargo run -- "Transcribe this" --record-audio --no-save # Custom prompt transcription

# Tool calling (function calling) - enable AI to use tools
cargo run -- --enable-tools --tool-allow-cwd "analyze this codebase and summarize the architecture"
cargo run -- --enable-tools --tool-allow-cwd "search the web for rust async best practices"
cargo run -- --enable-tools --tool-allowed-dir /tmp "create a summary of important files in /tmp"

# Tool calling with specific tools disabled
cargo run -- --enable-tools --tool-disable search_web --tool-allow-cwd "analyze files only"
cargo run -- --enable-tools --tool-disable write_file,execute_command --tool-allow-cwd "read-only analysis"

# Tool calling with command execution (dangerous - requires confirmation)
cargo run -- --enable-tools --allow-command-execution --confirm-commands --tool-allow-cwd "run git status and summarize changes"

# Web search with different modes
cargo run -- --enable-tools "search for latest Rust news"  # Gemini grounding (default, paid)
GIA_SEARCH_MODE=duckduckgo cargo run -- --enable-tools "search for AI news"  # Free DuckDuckGo
GIA_SEARCH_MODE=brave GIA_BRAVE_API_KEY=your_key cargo run -- --enable-tools "search"  # Free Brave
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

For Ollama: Install from https://ollama.ai and run `ollama serve`

### Default Model Configuration
Set default AI model:
```bash
# Set default model globally
export GIA_DEFAULT_MODEL="gemini-2.5-pro"

# Use Ollama model as default
export GIA_DEFAULT_MODEL="ollama::llama3.2"

# Windows
set GIA_DEFAULT_MODEL=gemini-2.5-pro
```

### Web Search Configuration (Tool Calling)
Configure search mode for tool calling:
```bash
# Default (no variable) → Gemini grounding with Google Search (PAID, ~$0.50 per 1000 searches)
# Best quality, automatic, includes citations, but costs money per search

# Free alternatives:
export GIA_SEARCH_MODE="duckduckgo"  # Free DuckDuckGo search
export GIA_SEARCH_MODE="brave"       # Free Brave (requires API key below)
export GIA_BRAVE_API_KEY="your_brave_api_key_here"  # Get free key at https://brave.com/search/api/

# Windows
set GIA_SEARCH_MODE=duckduckgo
# or
set GIA_SEARCH_MODE=brave
set GIA_BRAVE_API_KEY=your_brave_api_key_here
```

**Search Mode Options:**

| Mode | Provider | Cost | Quality | API Key Required |
|------|----------|------|---------|------------------|
| (default/unset) | **Gemini Grounding** | 💰 ~$0.0005/search (Flash)<br>~$0.005/search (Pro) | ⭐⭐⭐⭐⭐ Best | No (uses Gemini key) |
| `duckduckgo` | DuckDuckGo | 💚 Free | ⭐⭐⭐ Good | No |
| `brave` | Brave Search | 💚 Free (2000/month) | ⭐⭐⭐⭐ Better | Yes (free tier) |

**Recommendation:**
- **For best results**: Leave unset (Gemini grounding) - costs ~$0.05 per 100 searches
- **To save money**: Use `GIA_SEARCH_MODE=duckduckgo` (completely free)
- **For better free results**: Use `GIA_SEARCH_MODE=brave` with free API key

**Note for Ollama users**: Gemini grounding is not available for Ollama models. They automatically use DuckDuckGo (free) regardless of the setting.

### Logging
```bash
# Console logging (requires RUST_LOG to be set)
RUST_LOG=debug cargo run -- "test"  # Debug logging to stderr
RUST_LOG=info cargo run -- "test"   # Info logging to stderr
RUST_LOG=error cargo run -- "test"  # Error logging only to stderr

# File logging (per-conversation log files in ~/.gia/conversations/)
GIA_LOG_TO_FILE=1 cargo run -- "test"  # Enable file logging only
GIA_LOG_TO_FILE=1 RUST_LOG=info cargo run -- "test"  # Enable both file and console logging

# Without either variable, no logs are produced (clean stdout/stderr)
cargo run -- "test"  # No logging output
```

**Logging Behavior:**
- **Console logging**: Only enabled when `RUST_LOG` is set (outputs to stderr)
- **File logging**: Only enabled when `GIA_LOG_TO_FILE` is set (creates `<conversation-id>.log` in `~/.gia/conversations/`)
- **Both can be enabled simultaneously**: Set both environment variables
- **Clean output by default**: Without these variables, stdout remains clean for piping

## Architecture

### Workspace Structure
This is a Cargo workspace with shared dependencies and build configuration:
- **Workspace root**: Contains workspace `Cargo.toml` and shared resources (`icons/`)
- **gia/**: CLI binary crate
- **giagui/**: GUI binary crate
- Both crates share the same `build.rs` for git-based versioning
- Shared resources: `~/.gia/roles` and `~/.gia/tasks` (runtime, not in repo)

### Module Structure (gia CLI)
- `gia/src/main.rs` - CLI argument parsing, main application flow
- `gia/src/gemini.rs` - Gemini API client with rate limit fallback logic
- `gia/src/ollama.rs` - Ollama API client for local LLM support
- `gia/src/provider.rs` - Provider abstraction and factory
- `gia/src/api_key.rs` - API key management (multiple keys, validation, fallback)
- `gia/src/clipboard.rs` - Clipboard operations using arboard
- `gia/src/conversation.rs` - Conversation management with persistent storage
- `gia/src/logging.rs` - Structured logging to stderr

### Module Structure (giagui GUI)
- `giagui/src/main.rs` - Single-file egui application
- **Args struct**: Command-line argument parsing with clap
- **SpinnerApp struct**: Minimal spinner-only display mode
- **GiaApp struct**: Full GUI application state
- **Command execution**: Spawns `gia` CLI process
- **Icons**: References `../icons/gia.png` from workspace root
- **Spinner mode**: Launched with `--spinner` or `-s` flag, displays only animated spinner until process is killed

### Key Design Patterns

**Multi-API Key Support**: The application supports pipe-separated API keys (`key1|key2|key3`) and automatically falls back to alternative keys when encountering 429 rate limit errors. This is handled in `gemini.rs` with cooperation from `api_key.rs`.

**Input Sources**: Four input sources can be combined:
1. Command line arguments (primary prompt)
2. Clipboard content (with `-c` flag) - supports both text and images
3. Stdin content (automatically detected when available)
4. File content (with `-f` flag) - automatically detects media files vs text files, can be used multiple times

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

**API Key Fallback Algorithm** (implemented in `gemini.rs`):
1. **Initialization**: Read all API keys from `GEMINI_API_KEY` environment variable (pipe-separated: `key1|key2|key3`)
2. **Random Start**: Randomly select one key as the starting key
3. **API Request**: Make request using current key
4. **On 429 Rate Limit Error**: 
   - Log the rate limit hit
   - Show user message: `⚠️  Rate limit hit on API key. Trying next key... (X/Y)`
   - Move to next key using round-robin (modulo wrap-around)
   - Retry the request with the new key
5. **Cycle Detection**: Track the starting key index; if we cycle back to it, all keys have been tried
6. **All Keys Failed**: 
   - Log error with total attempts
   - Show user message: `❌ All X API keys exhausted. All keys have hit rate limits.`
   - Return error and stop processing
7. **Important**: The `GEMINI_API_KEY` environment variable is **never modified** at runtime; keys are passed directly to the API client via `AuthResolver`

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

### CLI (gia)
- All logging goes to stderr, leaving stdout clean for piping
- The tool validates API key format but continues with warnings if invalid
- Rate limit handling automatically tries alternative keys if available
- Windows-specific registry support was removed in favor of environment variables only
- Clipboard input automatically detects and handles both text and images
- When using `-c` flag, if an image is in the clipboard, it will be treated as an image input rather than text

### GUI (giagui)
- The `show_conversation()` method (Ctrl+O) spawns the GIA command without clearing or modifying the response box - do not change this behavior
- Focus management: Prompt input automatically receives focus on application start
- Icon handling: Application icon and logo are embedded from `../../icons/gia.png` (workspace root)
- Requires `gia` binary in PATH

### Build System
- Both `gia/build.rs` and `giagui/build.rs` are identical copies that generate version info from git commit count
- Git commands traverse up to find `.git` directory at workspace root
**Environment variables set: `GIA_VERSION`, `GIA_COMMIT_COUNT`, `GIA_IS_DIRTY`**

**Default Model Configuration**: Both CLI and GUI respect the `GIA_DEFAULT_MODEL` environment variable:
- **Default**: `gemini-2.5-flash-lite`
- **CLI**: Uses environment variable for `--model` flag default value
- **GUI**: Uses environment variable for model dropdown default selection
- **Supported formats**: Standard model names (`gemini-2.5-pro`) or provider format (`ollama::llama3.2`)

### GitHub Actions
- Workflow builds both `gia` and `giagui` for three platforms (Windows x64, macOS Intel, macOS ARM)
- Releases 6 binaries total: `gia-{platform}` and `giagui-{platform}` for each platform