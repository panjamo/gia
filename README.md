# GIA - Google Intelligence Assistant

A command-line tool that sends text prompts to Google's Gemini API and returns AI-generated responses.

## Features

- Read prompts from stdin, clipboard, or command line arguments
- Output responses to stdout or clipboard
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

## Usage

### Basic usage (stdin to stdout)
```bash
echo "What is Rust?" | gia
```

### Using command line prompt as prefix
```bash
# Prompt will be prepended to stdin input
echo "Machine learning algorithms" | gia --prompt "Explain in simple terms:"

# Prompt will be prepended to clipboard input  
gia -p "Summarize this text:" -i
```

### Clipboard operations
```bash
# Read prompt from clipboard, output to stdout
gia -i

# Read from stdin, output to clipboard
echo "Summarize this text" | gia -o

# Read from clipboard, output to clipboard
gia -i -o
```

### Command line options

- `-p, --prompt <TEXT>` - Prepend prompt text to input from stdin/clipboard
- `-i, --clipboard-input` - Read prompt from clipboard instead of stdin
- `-o, --clipboard-output` - Write response to clipboard instead of stdout

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
gia -p "What are the benefits of using Rust?"
```

### Code explanation
```bash
echo "fn main() { println!('Hello'); }" | gia -p "Explain this Rust code:"
```

### Working with clipboard
```bash
# Copy some text to clipboard first, then:
gia -p "Summarize this text:" -i

# Translate text from clipboard to clipboard
gia -p "Translate to Spanish:" -i -o
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