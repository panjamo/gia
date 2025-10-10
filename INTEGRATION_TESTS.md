# Integration Tests

This document describes the integration test suite for the GIA project, implementing the requirements from [Issue #10](https://github.com/panjamo/gia/issues/10).

## Overview

The integration tests are organized following Rust best practices with dedicated `tests/` directories for both the CLI and GUI components:

```
gia/tests/
  ├── cli_tests.rs         # Full CLI workflow tests
  └── common/
      └── mod.rs           # Shared test utilities

giagui/tests/
  ├── gui_integration.rs   # GUI workflow tests
  └── common/
      └── mod.rs           # Shared utilities
```

## Running Tests

### Basic Integration Tests (No API Required)

Run all integration tests that don't require external APIs:

```bash
# Run all non-ignored integration tests
cargo test --test cli_tests --test gui_integration

# Run specific test categories
cargo test --test cli_tests                    # CLI tests only
cargo test --test gui_integration               # GUI tests only

# Run specific tests
cargo test --test cli_tests test_help_output
cargo test --test gui_integration test_version_output
```

### API-Dependent Tests (Require Setup)

Some tests require actual API keys or local services and are marked with `#[ignore]`:

```bash
# Run API tests (requires GEMINI_API_KEY)
export GEMINI_API_KEY="your_api_key_here"
cargo test --test cli_tests -- --ignored

# Run Ollama tests (requires local Ollama server)
# Install Ollama from https://ollama.ai and run `ollama serve`
cargo test --test cli_tests test_ollama_integration -- --ignored
```

### All Tests Including Unit Tests

```bash
# Run all tests (unit + integration)
cargo test

# Run only unit tests
cargo test --lib

# Run only integration tests
cargo test --test cli_tests --test gui_integration
```

## Test Categories

### CLI Integration Tests (`gia/tests/cli_tests.rs`)

#### Command-Line Interface Tests
- **Help/Version Output**: Validates CLI help and version information
- **Argument Parsing**: Tests various flag combinations and error handling
- **File Input Handling**: Tests text files, image files, and directories
- **Output Options**: Tests clipboard, browser, and TTS output modes

#### Configuration Tests
- **Model Selection**: Tests Gemini and Ollama model formats
- **Role/Task Loading**: Tests role file loading and error handling
- **Environment Variables**: Tests API key handling and configuration

#### Workflow Tests (API Required - `#[ignore]`)
- **Gemini API Integration**: End-to-end API calls with real responses
- **Conversation Management**: Tests conversation creation and resumption
- **Ollama Integration**: Tests local Ollama model interactions

### GUI Integration Tests (`giagui/tests/gui_integration.rs`)

#### GUI Application Tests
- **Startup Tests**: Validates normal and spinner mode startup
- **Help/Version**: Tests GUI command-line options
- **Dependency Checks**: Ensures `gia` binary availability

#### Process Management Tests
- **Command Construction**: Tests `gia` command building
- **Process Spawning**: Tests background process execution
- **Environment Handling**: Tests environment variable management

#### Mock Server Tests
- **Ollama Model Fetching**: Tests with mock HTTP server
- **Response Handling**: Tests async response processing

## Test Utilities

### Common Utilities (`tests/common/mod.rs`)

Both CLI and GUI tests share similar utility patterns:

#### `TestConfig`
- Manages temporary directories for test isolation
- Provides paths to built binaries (debug/release/PATH)
- Creates test files and images
- Constructs commands with proper arguments

#### `MockEnvironment`
- Safely modifies environment variables for tests
- Automatically restores original values on drop
- Prevents test pollution between runs

#### Helper Functions
- `has_api_key()`: Checks for GEMINI_API_KEY availability
- `has_ollama()`: Tests Ollama server connectivity
- `is_headless()`: Detects headless environments (for GUI tests)

### Test Macros

```rust
// Skip tests when requirements aren't met
skip_without_api_key!();        // Requires GEMINI_API_KEY
skip_without_ollama!();          // Requires Ollama server
skip_if_headless!();             // Requires display (GUI tests)
skip_without_gia!(config);       // Requires gia binary
```

## Environment Requirements

### Basic Tests
- Rust toolchain
- Built `gia` and `giagui` binaries (or available in PATH)

### API Tests
- `GEMINI_API_KEY` environment variable set
- Valid Google AI API key from https://makersuite.google.com/app/apikey

### Ollama Tests
- Ollama installed and running on `localhost:11434`
- At least one model pulled (e.g., `ollama pull llama3.2`)

### GUI Tests
- Display available (not headless)
- GUI libraries installed (handled by eframe/egui)

## Test Isolation

### Temporary Resources
- Each test uses isolated temporary directories
- Test files are automatically cleaned up
- No persistent state between tests

### Environment Safety
- `MockEnvironment` ensures clean environment restoration
- `serial_test` crate prevents environment variable conflicts
- Tests don't modify global configuration

### Binary Dependencies
- Tests try multiple binary locations (target/debug, target/release, PATH)
- Graceful fallback when binaries aren't available
- Clear error messages for missing dependencies

## Debugging Tests

### Verbose Output
```bash
# Run with debug output
RUST_LOG=debug cargo test --test cli_tests test_help_output

# Show test output
cargo test --test cli_tests -- --nocapture

# Run single test with backtrace
RUST_BACKTRACE=1 cargo test --test cli_tests test_specific_test
```

### Manual Testing
```bash
# Test the actual binaries the tests use
cargo build
./target/debug/gia --help
./target/debug/giagui --help

# Test with same environment as integration tests
export GEMINI_API_KEY="test_key"
cargo run -p gia -- "test prompt"
```

## Continuous Integration

### GitHub Actions Compatibility
- Tests marked with `#[ignore]` are skipped in CI by default
- Headless GUI tests are automatically skipped
- Environment detection prevents false failures

### Local Development
```bash
# Run the same tests as CI
cargo test --test cli_tests --test gui_integration

# Include API tests if keys are available
if [ -n "$GEMINI_API_KEY" ]; then
    cargo test --test cli_tests -- --ignored
fi
```

## Adding New Tests

### CLI Tests
1. Add test function to `gia/tests/cli_tests.rs`
2. Use `TestConfig::new()` for setup
3. Use `skip_*` macros for requirements
4. Mark API tests with `#[ignore]`

### GUI Tests
1. Add test function to `giagui/tests/gui_integration.rs`
2. Use `skip_if_headless!()` for display-dependent tests
3. Test process spawning rather than GUI interaction
4. Mock external services when possible

### Example Test Structure
```rust
#[test]
fn test_new_feature() {
    let config = TestConfig::new();
    
    let output = config
        .gia_command()
        .arg("--new-feature")
        .arg("test-input")
        .output()
        .expect("Failed to execute gia");
    
    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("expected-output"));
}

#[test]
#[ignore] // Use `cargo test -- --ignored` to run
fn test_api_dependent_feature() {
    skip_without_api_key!();
    
    // Test implementation...
}
```

## Best Practices

1. **Test Isolation**: Each test should be independent and not rely on external state
2. **Clear Assertions**: Use descriptive assertion messages and check specific outputs
3. **Graceful Degradation**: Skip tests when requirements aren't met rather than failing
4. **Resource Cleanup**: Use RAII patterns and temporary directories for automatic cleanup
5. **Documentation**: Comment complex test scenarios and expected behaviors

This integration test suite ensures reliable end-to-end testing of the GIA project while maintaining compatibility with various development and CI environments.