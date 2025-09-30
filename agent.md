# GIA Development Agent Guide

## Project Context
GIA is a CLI tool for interacting with Google's Gemini API. It supports multimodal inputs (text, images, files), conversation management, and multi-API key fallback.

## Key Architecture Points

### Module Structure
- `main.rs` - CLI entry point and argument parsing
- `gemini.rs` - API client with rate limit fallback
- `api_key.rs` - Multi-key management and validation
- `clipboard.rs` - Clipboard I/O operations
- `conversation.rs` - Persistent conversation storage
- `logging.rs` - Structured stderr logging

### Critical Features
1. **Multi-API Key Support**: Pipe-separated keys with automatic fallback on 429 errors
2. **Input Sources**: CLI args, clipboard (text/images), stdin, text files
3. **Output Destinations**: stdout (default), clipboard
4. **Conversation Persistence**: JSON-based storage in `~/.gia/conversations/`
5. **Context Management**: Automatic truncation to maintain context window

## Development Workflow

### Standard Development Cycle
1. Make code changes
2. Run tests: `cargo test`
3. Fix clippy warnings: `cargo clippy --fix --allow-dirty`
4. Format code: `cargo fmt`
5. Build: `cargo build --release`

### Testing
- Use `serial_test` crate for tests that modify environment variables
- Tests cover: API key parsing, fallback logic, I/O handling, conversation management
- Run with output: `cargo test -- --nocapture`

### Important Conventions
- All logging goes to stderr (stdout reserved for output)
- API key validation continues with warnings if invalid format
- Windows registry support removed (env vars only)
- Clipboard auto-detects text vs images

## Common Tasks

### Adding New Features
1. Update relevant module(s)
2. Add corresponding tests
3. Update CLAUDE.md if user-facing
4. Run full quality workflow (clippy, fmt, test)

### API Changes
- Maintain backward compatibility with existing conversation format
- Update version in Cargo.toml if breaking changes
- Test multi-key fallback scenarios

### Error Handling
- Provide user-friendly messages for common errors (missing keys, auth failures, rate limits)
- Log technical details to stderr
- Exit codes should reflect error types