//! Integration tests for gia CLI
//!
//! These tests exercise the full CLI workflow from argument parsing
//! to API calls and output generation.

use std::str;

mod common;

use common::{MockEnvironment, TestConfig, has_api_key, has_ollama};

#[test]
fn test_help_output() {
    let config = TestConfig::new();
    let output = config
        .gia_command()
        .arg("--help")
        .output()
        .expect("Failed to execute gia --help");

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("AI CLI tool using Google Gemini API"));
    assert!(stdout.contains("--clipboard-input"));
    assert!(stdout.contains("--file"));
}

#[test]
fn test_version_output() {
    let config = TestConfig::new();
    let output = config
        .gia_command()
        .arg("--version")
        .output()
        .expect("Failed to execute gia --version");

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("gia"));
}

#[test]
fn test_error_without_prompt() {
    let config = TestConfig::new();
    let output = config
        .gia_command()
        .output()
        .expect("Failed to execute gia");

    assert!(!output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("No input content provided") || stderr.contains("error"));
}

#[test]
fn test_file_input_text() {
    let config = TestConfig::new();
    let file_path = config.create_temp_file("test.txt", "This is test content for analysis.");

    // Mock environment to avoid actual API calls
    let mut mock_env = MockEnvironment::new();
    mock_env.remove_var("GEMINI_API_KEY");

    let output = config
        .gia_command()
        .arg("Analyze this text")
        .arg("-f")
        .arg(&file_path)
        .output()
        .expect("Failed to execute gia with file input");

    // Should fail without API key, but should parse args correctly
    assert!(!output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("API key") || stderr.contains("GEMINI_API_KEY"));
}

#[test]
fn test_file_input_image() {
    let config = TestConfig::new();
    let image_path = config.create_test_image("test.png");

    // Mock environment to avoid actual API calls
    let mut mock_env = MockEnvironment::new();
    mock_env.remove_var("GEMINI_API_KEY");

    let output = config
        .gia_command()
        .arg("What do you see in this image?")
        .arg("-f")
        .arg(&image_path)
        .output()
        .expect("Failed to execute gia with image input");

    // Should fail without API key, but should parse args correctly
    assert!(!output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("API key") || stderr.contains("GEMINI_API_KEY"));
}

#[test]
fn test_multiple_file_inputs() {
    let config = TestConfig::new();
    let text_file = config.create_temp_file("doc.txt", "Documentation content.");
    let image_file = config.create_test_image("diagram.png");

    // Mock environment to avoid actual API calls
    let mut mock_env = MockEnvironment::new();
    mock_env.remove_var("GEMINI_API_KEY");

    let output = config
        .gia_command()
        .arg("Analyze these files")
        .arg("-f")
        .arg(&text_file)
        .arg("-f")
        .arg(&image_file)
        .output()
        .expect("Failed to execute gia with multiple files");

    // Should fail without API key, but should parse args correctly
    assert!(!output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("API key") || stderr.contains("GEMINI_API_KEY"));
}

#[test]
fn test_conversation_listing() {
    let config = TestConfig::new();

    let output = config
        .gia_command()
        .arg("--list-conversations")
        .output()
        .expect("Failed to execute gia --list-conversations");

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    // Should either show conversations table headers or indicate none exist
    assert!(
        stdout.contains("index")
            || stdout.contains("messages")
            || stdout.contains("No saved")
            || stdout.is_empty()
    );
}

#[test]
fn test_model_parameter() {
    let config = TestConfig::new();

    // Mock environment to avoid actual API calls
    let mut mock_env = MockEnvironment::new();
    mock_env.remove_var("GEMINI_API_KEY");

    let output = config
        .gia_command()
        .arg("Test prompt")
        .arg("-m")
        .arg("gemini-2.5-flash")
        .output()
        .expect("Failed to execute gia with model parameter");

    // Should fail without API key, but should parse args correctly
    assert!(!output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("API key") || stderr.contains("GEMINI_API_KEY"));
}

#[test]
fn test_ollama_model_format() {
    let config = TestConfig::new();

    let output = config
        .gia_command()
        .arg("Test prompt")
        .arg("-m")
        .arg("ollama::llama3.2")
        .output()
        .expect("Failed to execute gia with Ollama model");

    // Should fail if Ollama not available, but should parse args correctly
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        stderr.contains("Ollama")
            || stderr.contains("connection")
            || stderr.contains("11434")
            || !output.status.success()
    );
}

#[test]
fn test_role_parameter() {
    let config = TestConfig::new();

    // Mock environment to avoid actual API calls
    let mut mock_env = MockEnvironment::new();
    mock_env.remove_var("GEMINI_API_KEY");

    let output = config
        .gia_command()
        .arg("Test prompt")
        .arg("-t")
        .arg("nonexistent-role")
        .output()
        .expect("Failed to execute gia with role parameter");

    // Should handle missing role files gracefully
    assert!(!output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    // Could fail due to missing API key or missing role file
    assert!(
        stderr.contains("API key")
            || stderr.contains("GEMINI_API_KEY")
            || stderr.contains("role")
            || stderr.contains("not found")
    );
}

// Tests that require actual API keys - only run if available
#[test]
#[ignore] // Use `cargo test -- --ignored` to run these
fn test_gemini_api_integration() {
    if !has_api_key() {
        eprintln!("Skipping test: GEMINI_API_KEY not set");
        return;
    }

    let config = TestConfig::new();
    let output = config
        .gia_command()
        .arg("Say 'Hello from integration test'")
        .output()
        .expect("Failed to execute gia with API");

    if output.status.success() {
        let stdout = str::from_utf8(&output.stdout).unwrap();
        assert!(!stdout.is_empty());
        assert!(stdout.to_lowercase().contains("hello"));
    } else {
        let stderr = str::from_utf8(&output.stderr).unwrap();
        eprintln!("API test failed: {}", stderr);
        // Don't fail the test - API might be rate limited or temporarily unavailable
    }
}

#[test]
#[ignore] // Use `cargo test -- --ignored` to run these
fn test_conversation_resume() {
    if !has_api_key() {
        eprintln!("Skipping test: GEMINI_API_KEY not set");
        return;
    }

    let config = TestConfig::new();

    // First, create a conversation
    let output1 = config
        .gia_command()
        .arg("My name is TestBot. Remember this.")
        .output()
        .expect("Failed to execute first gia command");

    if !output1.status.success() {
        eprintln!("First command failed, skipping resume test");
        return;
    }

    // Then try to resume it
    let output2 = config
        .gia_command()
        .arg("--resume")
        .arg("What is my name?")
        .output()
        .expect("Failed to execute resume gia command");

    if output2.status.success() {
        let stdout = str::from_utf8(&output2.stdout).unwrap();
        assert!(stdout.to_lowercase().contains("testbot"));
    } else {
        eprintln!("Resume test inconclusive - API might be unavailable");
    }
}

#[test]
#[ignore] // Use `cargo test -- --ignored` to run these
fn test_ollama_integration() {
    if !has_ollama() {
        eprintln!("Skipping test: Ollama not available on localhost:11434");
        return;
    }

    let config = TestConfig::new();
    let output = config
        .gia_command()
        .arg("Say 'Hello from Ollama test'")
        .arg("-m")
        .arg("ollama::llama3.2")
        .output()
        .expect("Failed to execute gia with Ollama");

    if output.status.success() {
        let stdout = str::from_utf8(&output.stdout).unwrap();
        assert!(!stdout.is_empty());
    } else {
        let stderr = str::from_utf8(&output.stderr).unwrap();
        eprintln!("Ollama test failed: {}", stderr);
        // Model might not be available
        assert!(stderr.contains("model") || stderr.contains("not found"));
    }
}

#[test]
fn test_audio_recording_help() {
    let config = TestConfig::new();

    // Test that audio recording option is recognized
    let output = config
        .gia_command()
        .arg("--record-audio")
        .arg("--help")
        .output()
        .expect("Failed to execute gia with audio recording help");

    // Should show help instead of trying to record
    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("record-audio") || stdout.contains("audio"));
}

#[test]
fn test_browser_output_option() {
    let config = TestConfig::new();

    // Mock environment to avoid actual API calls
    let mut mock_env = MockEnvironment::new();
    mock_env.remove_var("GEMINI_API_KEY");

    let output = config
        .gia_command()
        .arg("Test prompt")
        .arg("--browser-output")
        .output()
        .expect("Failed to execute gia with browser output");

    // Should fail without API key, but should parse args correctly
    assert!(!output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("API key") || stderr.contains("GEMINI_API_KEY"));
}

#[test]
fn test_clipboard_output_option() {
    let config = TestConfig::new();

    // Mock environment to avoid actual API calls
    let mut mock_env = MockEnvironment::new();
    mock_env.remove_var("GEMINI_API_KEY");

    let output = config
        .gia_command()
        .arg("Test prompt")
        .arg("-o")
        .output()
        .expect("Failed to execute gia with clipboard output");

    // Should fail without API key, but should parse args correctly
    assert!(!output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("API key") || stderr.contains("GEMINI_API_KEY"));
}

#[test]
fn test_invalid_model_parameter() {
    let config = TestConfig::new();

    let output = config
        .gia_command()
        .arg("Test prompt")
        .arg("-m")
        .arg("invalid::model::format")
        .output()
        .expect("Failed to execute gia with invalid model");

    // Should handle invalid model format gracefully
    assert!(!output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(
        stderr.contains("model")
            || stderr.contains("invalid")
            || stderr.contains("format")
            || stderr.contains("API key")
            || stderr.contains("GEMINI_API_KEY")
    );
}

#[test]
fn test_stdin_input_simulation() {
    let config = TestConfig::new();

    // Mock environment to avoid actual API calls
    let mut mock_env = MockEnvironment::new();
    mock_env.remove_var("GEMINI_API_KEY");

    let mut command = config.gia_command();
    command.arg("Analyze this input");

    // We can't easily test stdin in integration tests, but we can test
    // that the command accepts the argument structure
    let output = command.output().expect("Failed to execute gia");

    // Should fail without API key, but should parse args correctly
    assert!(!output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("API key") || stderr.contains("GEMINI_API_KEY"));
}
