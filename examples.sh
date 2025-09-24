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
echo "1. Default usage (clipboard to clipboard):"
echo "Command: gia 'Summarize this text' (copy text to clipboard first)"
echo "# Copy some text to clipboard first, then run:"
echo "# ./target/release/gia 'Summarize this text'"
echo "# Or without prompt (just process clipboard data):"
echo "# ./target/release/gia"
echo
echo "----------------------------------------"

echo "2. Using stdin input with stdout output:"
echo "Command: echo 'Explain async/await in Rust' | gia 'Please explain' --stdin --stdout"
echo "Explain async/await in Rust" | ./target/release/gia "Please explain" --stdin --stdout
echo
echo "----------------------------------------"

echo "3. Code explanation with stdin:"
echo "Command: echo 'fn fibonacci(n: u32) -> u32 { if n <= 1 { n } else { fibonacci(n-1) + fibonacci(n-2) } }' | gia 'Explain this Rust code' --stdin --stdout"
echo "fn fibonacci(n: u32) -> u32 { if n <= 1 { n } else { fibonacci(n-1) + fibonacci(n-2) } }" | ./target/release/gia "Explain this Rust code" --stdin --stdout
echo
echo "----------------------------------------"

echo "4. Creative prompt with stdin:"
echo "Command: echo 'coding late at night' | gia 'Write a haiku about' --stdin --stdout"
echo "coding late at night" | ./target/release/gia "Write a haiku about" --stdin --stdout
echo
echo "----------------------------------------"

echo "5. Technical question with context:"
echo "Command: echo 'Vec and HashMap' | gia 'What are the main differences between these Rust types' --stdin --stdout"
echo "Vec and HashMap" | ./target/release/gia "What are the main differences between these Rust types" --stdin --stdout
echo
echo "----------------------------------------"

echo "6. Using debug logging:"
echo "Command: echo 'world' | RUST_LOG=debug gia 'Hello' --stdin --stdout"
echo "world" | RUST_LOG=debug ./target/release/gia "Hello" --stdin --stdout 2>/dev/null
echo
echo "----------------------------------------"

# Clipboard examples (commented out as they require user interaction)
echo "Clipboard examples (default behavior - uncomment to test):"
echo "# Copy some text to clipboard first, then run:"
echo "# ./target/release/gia 'Summarize this'"
echo "# ./target/release/gia 'Translate to Spanish'"
echo "# ./target/release/gia                        # No prompt, just process clipboard"
echo "# ./target/release/gia 'Explain this concept' --stdout  # Output to terminal"
echo

echo "=== Examples completed ==="
echo
echo "To run with different log levels:"
echo "echo 'input' | RUST_LOG=info gia 'your prompt' --stdin --stdout"
echo "echo 'input' | RUST_LOG=debug gia 'your prompt' --stdin --stdout"
echo
echo "Default clipboard operations:"
echo "gia 'Explain this'           # Clipboard input, clipboard output (default)"
echo "gia                          # Process clipboard without additional prompt"
echo "gia 'Summarize' --stdout     # Clipboard input, stdout output"
echo "echo 'text' | gia 'Process this' --stdin  # Stdin input, clipboard output"