#!/bin/bash

# GIA Examples Script
# This script demonstrates various ways to use the gia CLI tool

echo "=== GIA Examples ==="
echo

# Check if GEMINI_API_KEY is set
if [ -z "$GEMINI_API_KEY" ]; then
    echo "Error: GEMINI_API_KEY environment variable not set"
    echo "Please set it with: export GEMINI_API_KEY='your_api_key_here'"
    exit 1
fi

# Build the project first
echo "Building gia..."
cargo build --release
echo

# Basic examples
echo "1. Prompt with stdin input:"
echo "Command: echo 'programming language' | gia -p 'What is Rust'"
echo "programming language" | ./target/release/gia -p "What is Rust"
echo
echo "----------------------------------------"

echo "2. Using stdin input:"
echo "Command: echo 'Explain async/await in Rust' | gia"
echo "Explain async/await in Rust" | ./target/release/gia
echo
echo "----------------------------------------"

echo "3. Code explanation example:"
echo "Command: echo 'fn fibonacci(n: u32) -> u32 { if n <= 1 { n } else { fibonacci(n-1) + fibonacci(n-2) } }' | gia -p 'Explain this Rust code:'"
echo "fn fibonacci(n: u32) -> u32 { if n <= 1 { n } else { fibonacci(n-1) + fibonacci(n-2) } }" | ./target/release/gia -p "Explain this Rust code:"
echo
echo "----------------------------------------"

echo "4. Creative prompt with stdin:"
echo "Command: echo 'coding late at night' | gia -p 'Write a haiku about:'"
echo "coding late at night" | ./target/release/gia -p "Write a haiku about:"
echo
echo "----------------------------------------"

echo "5. Technical question with context:"
echo "Command: echo 'Vec and HashMap' | gia -p 'What are the main differences between these Rust types:'"
echo "Vec and HashMap" | ./target/release/gia -p "What are the main differences between these Rust types:"
echo
echo "----------------------------------------"

echo "6. Using debug logging:"
echo "Command: echo 'world' | RUST_LOG=debug gia -p 'Hello'"
echo "world" | RUST_LOG=debug ./target/release/gia -p "Hello" 2>/dev/null
echo
echo "----------------------------------------"

# Clipboard examples (commented out as they require user interaction)
echo "Clipboard examples (uncomment to test):"
echo "# Copy some text to clipboard first, then run:"
echo "# gia -p 'Summarize:' -i"
echo "# gia -p 'Translate to Spanish:' -i -o"
echo

echo "=== Examples completed ==="
echo
echo "To run with different log levels:"
echo "echo 'input' | RUST_LOG=info gia -p 'your prompt:'"
echo "echo 'input' | RUST_LOG=debug gia -p 'your prompt:'"
echo
echo "For clipboard operations:"
echo "gia -p 'Explain:' -i     # Prompt + clipboard input"
echo "echo 'text' | gia -o    # Stdin to clipboard"