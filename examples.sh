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
echo "1. Default usage (command line prompt to stdout):"
echo "Command: gia 'What is artificial intelligence?'"
./target/release/gia "What is artificial intelligence?"
echo
echo "----------------------------------------"

echo "2. Using additional clipboard input:"
echo "Command: gia 'Explain this code' -c (copy code to clipboard first)"
echo "# Copy some code to clipboard first, then run:"
echo "# ./target/release/gia 'Explain this code' -c"
echo
echo "----------------------------------------"

echo "3. Using additional stdin input:"
echo "Command: echo 'fn fibonacci(n: u32) -> u32 { if n <= 1 { n } else { fibonacci(n-1) + fibonacci(n-2) } }' | gia 'Explain this Rust code' -s"
echo "fn fibonacci(n: u32) -> u32 { if n <= 1 { n } else { fibonacci(n-1) + fibonacci(n-2) } }" | ./target/release/gia "Explain this Rust code" -s
echo
echo "----------------------------------------"

echo "4. Output to clipboard:"
echo "Command: gia 'Write a haiku about programming' -o"
./target/release/gia "Write a haiku about programming" -o
echo "# Output written to clipboard"
echo
echo "----------------------------------------"

echo "5. Combining clipboard and stdin input:"
echo "Command: echo 'extra context' | gia 'Main question about this topic' -c -s"
echo "# Copy some text to clipboard first, then run:"
echo "# echo 'extra context' | ./target/release/gia 'Main question about this topic' -c -s"
echo
echo "----------------------------------------"

echo "6. Using debug logging:"
echo "Command: RUST_LOG=debug gia 'What are the main features of Rust?'"
RUST_LOG=debug ./target/release/gia "What are the main features of Rust?" 2>/dev/null
echo
echo "----------------------------------------"


echo
echo "----------------------------------------"

# Additional examples (commented out as they require user interaction)
echo "Additional examples (uncomment to test):"
echo "# Copy some text to clipboard first, then run:"
echo "# ./target/release/gia 'Summarize this text' -c"
echo "# ./target/release/gia 'Translate to Spanish' -c -o"
echo "# ./target/release/gia 'Fix any errors in this code' -c"
echo

echo "=== Examples completed ==="
echo
echo "To run with different log levels:"
echo "RUST_LOG=info gia 'your prompt here'"
echo "RUST_LOG=debug gia 'your prompt here'"
echo
echo "Basic operations:"
echo "gia 'Your question here'               # Direct AI question (default)"
echo "gia 'Explain this' -c                  # Add clipboard input"
echo "echo 'data' | gia 'Analyze this' -s   # Add stdin input"
echo "gia 'Write a poem' -o                  # Output to clipboard"
echo "gia 'Translate this' -c -o             # Clipboard input, clipboard output"