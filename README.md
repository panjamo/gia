# GIA - Google Intelligence Assistant

A command-line tool that sends text prompts to Google's Gemini API and returns AI-generated responses.

## Features

- Uses command line arguments as the main prompt
- **Audio recording** - Record audio prompts using ffmpeg with `-a` flag
- **Media file support** - Include images and audio/video files
  - `-i` flag: Media files only (JPEG, PNG, WebP, HEIC, PDF, OGG, OPUS, MP3, M4A, MP4)
  - `-f` flag: Text files (any extension)
- Optional additional input from clipboard or stdin (auto-detects text vs images)
- Output responses to stdout (default) or clipboard
- Persistent conversation history with resume capability
- Multi-API key support with automatic fallback
- Comprehensive logging to stderr
- Simple and fast CLI interface

## Installation

1. Clone this repository
2. Install Rust if you haven't already
3. For audio recording support, install ffmpeg:
   - **Windows**: Download from https://ffmpeg.org/download.html and add to PATH
   - **macOS**: `brew install ffmpeg`
   - **Linux**: `sudo apt install ffmpeg` (Ubuntu/Debian) or equivalent for your distribution
4. Build the project:
   ```
   cargo build --release
   ```

## Setup

Set your Google Gemini API key as an environment variable:

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

On Windows:
```cmd
set GEMINI_API_KEY=key1|key2|key3
```

Optionally configure the context window limit (default: 8000):

```bash
export CONTEXT_WINDOW_LIMIT=10000
```

For audio recording, optionally set your preferred audio device:

**Windows:**
```cmd
set GIA_AUDIO_DEVICE=Headset (WH-1000XM2)
```

**macOS:**
```bash
# Use device index (0, 1, 2, etc.) - run with RUST_LOG=debug to see available devices
export GIA_AUDIO_DEVICE="0"
```

**Linux:**
```bash
# Use device name or "default"
export GIA_AUDIO_DEVICE="default"
```

GIA will randomly select an API key for each request and automatically fallback to other keys if it encounters a "429 Too Many Requests" error.

To get a Gemini API key, visit: https://makersuite.google.com/app/apikey

## Default Behavior

GIA automatically combines input from multiple sources:
- **Command line**: Main prompt (required, except when using `-a` alone)
- **Audio recording**: With `-a` flag (requires ffmpeg)
- **Stdin**: Automatically detected when piped
- **Clipboard**: With `-c` flag only
- **Text files**: With `-f` flag (any extension)
- **Media files**: With `-i` flag (media only) or `-f` flag (text)
- **Output**: Response written to stdout (default)

## Usage

### Basic usage (command line prompt to stdout - default)
```bash
# Direct AI questions:
gia "What is artificial intelligence?"
gia "Explain quantum computing"

# Audio recording (auto-generates prompt):
gia --record-audio
gia -a  # Short option

# Audio recording with custom prompt:
gia --record-audio "Transcribe and summarize this audio"

# With clipboard input:
gia "Summarize this text" -c

# With stdin input (automatic):
echo "data to process" | gia "Analyze this data"
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

# Include audio/video files:
gia "Transcribe this recording" -i meeting.mp3
gia "What is discussed in this video?" -i presentation.mp4

# Combine multiple input sources:
gia "Analyze code and docs" -f README.md -f main.rs -i diagram.png
gia "Analyze audio and images" -i recording.mp3 -i screenshot.png
```

### Image analysis
```bash
# Analyze a single image:
gia "What do you see in this image?" -i photo.jpg

# Compare multiple images:
gia "What are the differences between these images?" -i image1.jpg -i image2.png

# Analyze image from clipboard (copy image first):
gia "What do you see in this image?" -c

# Combine file image with clipboard text:
gia "Explain this diagram" -i diagram.png -c

# Mix clipboard image with additional text prompt:
gia "Describe the technical aspects of this screenshot" -c

# Image with stdin input:
echo "Focus on the technical aspects" | gia "Analyze this screenshot" -i screenshot.png
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

```bash
# Resume latest conversation:
gia --resume "continue our discussion"
gia -R "continue our discussion"  # Short option

# Resume specific conversation by full ID:
gia --resume a1b2c3d4-e5f6-7890-abcd-ef1234567890 "follow up question"

# List all saved conversations:
gia --list-conversations
gia -l 5                          # List top 5 conversations
gia -l                            # List all conversations

# Display conversation (follows normal output options):
gia -s                            # Show latest conversation (stdout)
gia -s a1b2c3d4-e5f6-7890-abcd   # Show specific conversation (stdout)
gia -s -o                         # Show latest conversation (clipboard)
gia -s -b                         # Show latest conversation (file + browser)
```

### Command line options

- `[PROMPT_TEXT]` - Prompt text for the AI (main input)
- `-a, --record-audio` - Record audio input using ffmpeg (auto-generates prompt if no text provided)
- `-c, --clipboard-input` - Add clipboard content to prompt (auto-detects images vs text)
- `-i, --image <FILE>` - Add media file to prompt (can be used multiple times; JPEG, PNG, WebP, HEIC, PDF, OGG, OPUS, MP3, M4A, MP4)
- `-f, --file <FILE>` - Add text or media file to prompt (text files with any extension)
- `-o, --clipboard-output` - Write response to clipboard instead of stdout
- `-b, --browser-output` - Write output to file (~/.gia/outputs/, path copied to clipboard) AND open browser preview
- `-r, --resume [ID]` - Resume last conversation or specify conversation ID
- `-R` - Resume the very last conversation
- `-l, --list-conversations [NUMBER]` - List saved conversations (optionally limit number)
- `-s, --show-conversation [ID]` - Show conversation (follows output options: stdout/clipboard/file+browser)
- `-m, --model <MODEL>` - Specify Gemini model (default: gemini-2.5-flash-lite) see https://ai.google.dev/gemini-api/docs/models

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
- `genai` - Gemini API client
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
