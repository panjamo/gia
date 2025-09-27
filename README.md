# GIA - Google Intelligence Assistant

A command-line tool that sends text prompts to Google's Gemini API and returns AI-generated responses.

## Features

- Uses command line arguments as the main prompt
- **Image support** - Include images from files (JPEG, PNG, WebP, HEIC, PDF) or clipboard
- Optional additional input from clipboard or stdin (auto-detects text vs images)
- Output responses to stdout (default) or clipboard
- Persistent conversation history with resume capability
- Multi-API key support with automatic fallback
- Comprehensive logging to stderr
- Simple and fast CLI interface

## Installation

1. Clone this repository
2. Install Rust if you haven't already
3. Build the project:
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

GIA will randomly select an API key for each request and automatically fallback to other keys if it encounters a "429 Too Many Requests" error.

To get a Gemini API key, visit: https://makersuite.google.com/app/apikey

## Default Behavior

GIA uses command line arguments as the main prompt:
- **Input**: Prompt from command line arguments (required)
- **Additional Input**: Optional clipboard (-c) or stdin (-s) content
- **Output**: Response written to stdout (default)

Use flags to add additional input or redirect output.

## Usage

### Basic usage (command line prompt to stdout - default)
```bash
# Direct AI questions:
gia "What is artificial intelligence?"
gia "Explain quantum computing"

# With additional clipboard input:
gia "Summarize this text" -c

# With additional stdin input:
echo "data to process" | gia "Analyze this data" -s
```

### Adding input sources
```bash
# Add clipboard content to prompt:
gia "Explain this code" -c

# Add stdin content to prompt:
echo "machine learning data" | gia "Analyze this" -s

# Combine both:
echo "extra context" | gia "Main question about this topic" -c -s
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
echo "Focus on the technical aspects" | gia "Analyze this screenshot" -i screenshot.png -s
```

### Output options
```bash
# Default stdout output:
gia "What is machine learning?"

# Output to clipboard instead:
gia "Write a poem about coding" -o

# Output to file (~/.gia/outputs/) AND open browser preview:
gia "Generate markdown documentation" -O

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

# Display conversation in browser (chat format):
gia -v                            # Show latest conversation
gia -v a1b2c3d4-e5f6-7890-abcd   # Show specific conversation
```

### Command line options

- `[PROMPT_TEXT]` - Prompt text for the AI (main input)
- `-c, --clipboard-input` - Add clipboard content to prompt (auto-detects images vs text)
- `-s, --stdin` - Add stdin content to prompt
- `-i, --image <FILE>` - Add image file to prompt (can be used multiple times; supports JPEG, PNG, WebP, HEIC, PDF)
- `-o, --clipboard-output` - Write response to clipboard instead of stdout
- `-O` - Write output to file (~/.gia/outputs/, path copied to clipboard) AND open browser preview
- `-r, --resume [ID]` - Resume last conversation or specify conversation ID
- `-R` - Resume the very last conversation
- `-l, --list-conversations [NUMBER]` - List saved conversations (optionally limit number)
- `-v, --show-conversation [ID]` - Show conversation in chat mode as HTML/Markdown (latest if no ID)
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
echo "fn main() { println!('Hello'); }" | gia "Explain this Rust code" -s
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
