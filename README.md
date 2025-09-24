# GIA - Google Intelligence Assistant

A command-line tool that sends text prompts to Google's Gemini API and returns AI-generated responses.

## Features

- Read prompts from command line and data from clipboard (default) or stdin
- Output responses to clipboard (default) or stdout
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

To get a Gemini API key, visit: https://makersuite.google.com/app/apikey

## Default Behavior

GIA uses clipboard by default for both input and output:
- **Input**: Prompt from command line + data from clipboard
- **Output**: Response written to clipboard

Use flags to override defaults for stdin/stdout.

## Usage

### Basic usage (clipboard to clipboard - default)
```bash
# Copy data to clipboard first, then:
gia "Summarize this text"

# Or without prompt (just process clipboard data):
gia

# Prompt-only mode (no additional input):
gia -p "What is artificial intelligence?"
```

### Using stdin/stdout
```bash
echo "What is Rust?" | gia "Explain this" --stdin --stdout
```

### Prompt-only mode
```bash
# Direct question to AI (no additional input)
gia -p "What are the benefits of functional programming?"

# Works with output redirection
gia -p "Write a haiku about coding" --stdout
```

### Mixed operations
```bash
# Clipboard input, stdout output
gia "Translate to Spanish" --stdout

# Stdin input, clipboard output (default output)
echo "Machine learning algorithms" | gia "Explain in simple terms" --stdin
```

### Command line options

- `[PROMPT_TEXT]` - Optional prompt text for the AI (prepended to input data)
- `-p, --prompt-only` - Use only command-line arguments as prompt (no stdin/clipboard input)
- `-s, --stdin` - Read input data from stdin instead of clipboard
- `-t, --stdout` - Write response to stdout instead of clipboard

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
# Copy question to clipboard, then:
gia "What are the benefits of using Rust for this project?"

# Or copy complete question to clipboard and just run:
gia

# Direct question without any input:
gia -p "What are the benefits of using Rust?"
```

### Code explanation
```bash
echo "fn main() { println!('Hello'); }" | gia "Explain this Rust code" --stdin --stdout
```

### Working with clipboard (default)
```bash
# Copy some text to clipboard first, then:
gia "Summarize this text"

# Process clipboard content without additional prompt:
gia

# Translate text from clipboard to clipboard
gia "Translate to Spanish"

# Get output to terminal instead
gia "Explain this concept" --stdout

# Ask direct questions
gia -p "How does machine learning work?"
```

## Dependencies

- `tokio` - Async runtime
- `reqwest` - HTTP client for Gemini API
- `serde` - JSON serialization
- `clap` - Command line parsing
- `anyhow` - Error handling
- `log` + `env_logger` - Logging
- `arboard` - Clipboard operations

## License

MIT License