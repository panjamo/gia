<img src="icons/gia.png" alt="gia" width="128" height="128">

# GIA - General Intelligence Assistant

A command-line tool (and GUI wrapper) that sends text prompts to AI models and returns AI-generated responses.

This workspace contains two binaries:
- **gia** - Command-line interface
- **giagui** - GUI wrapper for gia

## Features

- Uses command line arguments as the main prompt
- **Roles & Tasks** - Load AI role definitions and task instructions from markdown files
- **Audio recording** - Record audio prompts natively with `-a` flag (no external dependencies)
- **Smart file support** - Include any files or directories
  - `-f` flag: Automatically detects media files (JPEG, PNG, WebP, HEIC, PDF, OGG, OPUS, MP3, M4A, MP4) vs text files
  - Supports directories (processes all files recursively with auto-detection)
- Optional additional input from clipboard or stdin (auto-detects text vs images)
- Output responses to stdout (default) or clipboard
- Persistent conversation history with resume capability
- Multi-API key support with automatic fallback
- Comprehensive logging to stderr
- Simple and fast CLI interface

📊 **[View Input/Output Flow Diagram](gia-flow-diagram.md)** - Visual overview of all input sources and output destinations

## Installation

1. Clone this repository
2. Install Rust if you haven't already
3. Build the project:
   ```bash
   # Build both binaries
   cargo build --release
   
   # Or build specific binaries
   cargo build --release -p gia      # CLI only
   cargo build --release -p giagui   # GUI only
   ```
   
   Binaries will be located at:
   - `target/release/gia` (or `gia.exe` on Windows)
   - `target/release/giagui` (or `giagui.exe` on Windows)

## Setup

### Using Gemini (Google AI)

Set your API key as an environment variable:

```bash
export GEMINI_API_KEY="your_api_key_here"
```

On Windows:
```cmd
set GEMINI_API_KEY=your_api_key_here
```

For automatic fallback on rate limits, set multiple keys separated by pipe (`|`):

```bash
export GEMINI_API_KEY="key1|key2|key3"
```

To get an API key, visit: https://makersuite.google.com/app/apikey

### Using Ollama (Local Models)

Install and start Ollama from https://ollama.ai, then use the `-m` flag:

```bash
gia -m "ollama::llama3.2" "your prompt here"
```

### Optional Configuration

Configure the default AI model (default: gemini-2.5-flash-lite):

```bash
# Set default model globally
export GIA_DEFAULT_MODEL="gemini-2.5-pro"

# Use Ollama model as default
export GIA_DEFAULT_MODEL="ollama::llama3.2"
```

**Windows:**
```cmd
set GIA_DEFAULT_MODEL=gemini-2.5-pro
set GIA_DEFAULT_MODEL=ollama::llama3.2
```

Configure the context window limit (default: 8000):

```bash
export CONTEXT_WINDOW_LIMIT=10000
```



## Environment Variables & Help

### Environment Variables
- `GEMINI_API_KEY` - Gemini API key(s), pipe-separated for fallback: `key1|key2|key3`
- `GIA_DEFAULT_MODEL` - Default AI model (default: `gemini-2.5-flash-lite`)
- `GIA_AUDIO_DEVICE` - Default audio input device for recording
- `CONTEXT_WINDOW_LIMIT` - Context window size limit (default: 8000)
- `RUST_LOG` - Logging level: `debug`, `info`, `error` (outputs to stderr)
- `GIA_LOG_TO_FILE` - Enable per-conversation file logging: `1`

### Getting Help
```bash
gia --help          # Full help with all options and examples
gia -h              # Short help with basic usage
```

## Default Behavior

GIA automatically combines input from multiple sources:
- **Command line**: Main prompt (required, except when using `-a` alone)
- **Audio recording**: With `-a` flag (native recording, no external dependencies)
- **Stdin**: Automatically detected when piped
- **Clipboard**: With `-c` flag only
- **Text files**: With `-f` flag (any extension)
- **Files**: With `-f` flag (auto-detects media vs text files)
- **Output**: Response written to stdout (default)

## GUI Usage (giagui)

The GUI provides a simple interface to interact with GIA:

**Features:**
- Multi-line prompt input
- Custom options field
- Clipboard input toggle (`-c`)
- Browser output toggle (`--browser-output`)
- Auto-resume conversations after first prompt
- Response display with copy to clipboard
- Show conversation in browser (Ctrl+O)
- Audio recording support (Ctrl+R)

**Keyboard Shortcuts:**
- **Ctrl+Enter**: Send prompt
- **Ctrl+R**: Send with audio recording
- **Ctrl+L**: Clear form
- **Ctrl+Shift+C**: Copy response to clipboard
- **Ctrl+O**: Show conversation in browser
- **F1**: Show help

**Requirements:**
- `gia` must be installed and available in PATH

**Running:**
```bash
cargo run -p giagui
# or after building
./target/release/giagui
```

## CLI Usage (gia)

### Basic usage (command line prompt to stdout - default)
```bash
# Direct AI questions:
gia "What is artificial intelligence?"
gia "Explain quantum computing"

# With roles/tasks:
gia -t rust-dev "Explain this code" -c
gia -t code-review -t security-audit "Review this PR"

# Audio recording (auto-generates prompt):
gia --record-audio
gia -a  # Short option

# Audio recording with custom prompt:
gia --record-audio "Transcribe and summarize this audio"

# Audio recording with specific device:
gia --list-audio-devices                                    # List available devices
gia --audio-device "Microphone Array" --record-audio        # Use specific device
GIA_AUDIO_DEVICE="Microphone Array" gia --record-audio     # Use env var

# Transcribe-only mode (no conversation history saved):
gia --record-audio --role EN --no-save         # English transcription only
gia --record-audio --role DE --no-save         # German transcription only
gia "Transcribe this" --record-audio --no-save # Custom prompt transcription

# With clipboard input:
gia "Summarize this text" -c

# With stdin input (automatic):
echo "data to process" | gia "Analyze this data"
```

### Roles & Tasks
```bash
# Create role/task files:
# ~/.gia/roles/rust-dev.md - AI persona definitions
# ~/.gia/tasks/code-review.md - Specific task instructions

# Use roles/tasks (searches roles/ first, then tasks/):
gia -t rust-dev "Optimize this function" -c
gia -t code-review -t security-audit "Review changes"
```

### Adding input sources
```bash
# Add clipboard content to prompt:
gia "Explain this code" -c

# Stdin is automatically detected:
echo "machine learning data" | gia "Analyze this"

# Combine stdin and clipboard:
echo "extra context" | gia "Main question about this topic" -c

# Include text files:
gia "Summarize these documents" -f doc1.txt -f doc2.txt

# Include entire directories (processes all files recursively):
gia "Analyze the codebase" -f src/
gia "Review all documentation" -f docs/ -f README.md

# Include audio/video files (auto-detected as media):
gia "Transcribe this recording" -f meeting.mp3
gia "What is discussed in this video?" -f presentation.mp4

# Combine multiple input sources (auto-detection):
gia "Analyze code, docs, and diagram" -f README.md -f main.rs -f diagram.png
gia "Analyze audio and images" -f recording.mp3 -f screenshot.png
```

### Image analysis
```bash
# Analyze a single image (auto-detected):
gia "What do you see in this image?" -f photo.jpg

# Compare multiple images (auto-detected):
gia "What are the differences between these images?" -f image1.jpg -f image2.png

# Analyze image from clipboard (copy image first):
gia "What do you see in this image?" -c

# Combine file image with clipboard text:
gia "Explain this diagram" -f diagram.png -c

# Mix clipboard image with additional text prompt:
gia "Describe the technical aspects of this screenshot" -c

# Image with stdin input:
echo "Focus on the technical aspects" | gia "Analyze this screenshot" -f screenshot.png
```

### Output options
```bash
# Default stdout output:
gia "What is machine learning?"

# Output to clipboard instead:
gia "Write a poem about coding" -o

# Output to file (~/.gia/outputs/) AND open browser preview:
gia "Generate markdown documentation" -b

# With additional input and clipboard output:
gia "Translate to Spanish" -c -o
```

### Conversation Management

Conversations are saved in `~/.gia/conversations/` with consistent naming:
- JSON: `conversation-slug-abc1.json`
- Markdown: `conversation-slug-abc1.md`
- Output files: `conversation-slug-abc1_20250107_143022.md`

```bash
# Resume latest conversation:
gia --resume "continue our discussion"
gia -R "continue our discussion"  # Short option

# Resume by index (from -l list), ID, or hash:
gia --resume 0 "continue"        # Index 0 = newest conversation
gia --resume 2 "follow up"       # Index 2 from list
gia --resume conversation-slug-abc1 "follow up"
gia --resume abc1 "follow up"    # Match by 4-char hash

# List all saved conversations (tabular output):
gia --list-conversations
gia -l 5                          # List top 5 conversations
gia -l                            # List all conversations

# Display conversation (follows normal output options):
gia -s                            # Show latest conversation (stdout)
gia -s 0                          # Show newest (index 0)
gia -s abc1                       # Show by hash (stdout)
gia -s -o                         # Show latest conversation (clipboard)
gia -s -b                         # Show latest conversation (file + browser)
```

### Command line options

- `[PROMPT_TEXT]` - Prompt text for the AI (main input)
- `-t, --role <NAME>` - Load role/task from ~/.gia/roles/ or ~/.gia/tasks/ (can be used multiple times)
- `-a, --record-audio` - Record audio input natively (auto-generates prompt if no text provided)
- `--audio-device <DEVICE>` - Specify audio input device for recording (overrides GIA_AUDIO_DEVICE)
- `--list-audio-devices` - List all available audio input devices and exit
- `-c, --clipboard-input` - Add clipboard content to prompt (auto-detects images vs text)
- `-f, --file <FILE_OR_DIR>` - Add file or directory to prompt (auto-detects media vs text; directories processed recursively)
- `-o, --clipboard-output` - Write response to clipboard instead of stdout
- `-b, --browser-output` - Write output to file (~/.gia/outputs/, path copied to clipboard) AND open browser preview
- `-r, --resume [ID]` - Resume last conversation or specify conversation ID
- `-R` - Resume the very last conversation
- `-l, --list-conversations [NUMBER]` - List saved conversations (optionally limit number)
- `-s, --show-conversation [ID]` - Show conversation (follows output options: stdout/clipboard/file+browser)
- `-m, --model <MODEL>` - Specify model (default: gemini-2.5-flash-lite)
- `--no-save` - Don't save to conversation history (transcribe-only mode)
  - Gemini models: see https://ai.google.dev/gemini-api/docs/models
  - Ollama models: use `ollama::model-name` format (e.g., `ollama::llama3.2`)

#### Audio Device Selection Priority
Device selection follows this priority (highest to lowest):
1. `--audio-device` CLI parameter
2. `GIA_AUDIO_DEVICE` environment variable
3. Default system audio input device

## Logging

Logging is written to stderr with different levels:
- Set `RUST_LOG=debug` for detailed logs
- Set `RUST_LOG=info` for general information
- Set `RUST_LOG=error` for errors only

Example:
```bash
RUST_LOG=debug gia -p "Hello world"
```

## Examples

### Simple question
```bash
# Direct questions:
gia "What are the benefits of using Rust?"
gia "How does machine learning work?"
gia "Write a haiku about programming"
```

### Code explanation
```bash
# Copy code to clipboard first, then:
gia "Explain this Rust code" -c

# Or pipe code via stdin:
echo "fn main() { println!('Hello'); }" | gia "Explain this Rust code"
```

### Working with clipboard
```bash
# Copy text to clipboard first, then add it to your prompt:
gia "Summarize this text" -c
gia "Translate to Spanish" -c
gia "Fix any errors in this code" -c

# Output to clipboard instead of stdout:
gia "Rewrite this professionally" -c -o
```

## Dependencies

- `tokio` - Async runtime
- `genai` - AI API client (Gemini, Ollama)
- `serde` - JSON serialization
- `clap` - Command line parsing
- `anyhow` - Error handling
- `log` + `env_logger` - Logging
- `arboard` - Clipboard operations (text and images)
- `image` - Image processing and PNG conversion
- `webbrowser` - Browser opening
- `comrak` - Markdown to HTML rendering
- `base64` - Base64 encoding for data URLs

## License

MIT License
