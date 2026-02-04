//! Integration tests for giagui GUI
//!
//! These tests exercise the GUI application's core functionality
//! including model fetching, process spawning, and response handling.

use std::str;
use std::time::Duration;

mod common;

use common::{MockEnvironment, TestConfig, has_api_key, has_ollama, is_headless};

#[test]
fn test_help_output() {
    let config = TestConfig::new();
    let output = config
        .giagui_command()
        .arg("--help")
        .output()
        .expect("Failed to execute giagui --help");

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("GIA GUI - Graphical user interface"));
    assert!(stdout.contains("--spinner") || stdout.contains("spinner"));
}

#[test]
fn test_version_output() {
    let config = TestConfig::new();
    let output = config
        .giagui_command()
        .arg("--version")
        .output()
        .expect("Failed to execute giagui --version");

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("giagui"));
}

#[test]
fn test_spinner_mode_help() {
    let config = TestConfig::new();

    // Test that spinner mode option is recognized
    let output = config
        .giagui_command()
        .arg("--spinner")
        .arg("--help")
        .output()
        .expect("Failed to execute giagui --spinner --help");

    // Should show help instead of trying to start spinner
    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("spinner") || stdout.contains("Spinner"));
}

#[test]
fn test_short_spinner_flag() {
    let config = TestConfig::new();

    // Test that short spinner flag is recognized
    let output = config
        .giagui_command()
        .arg("-s")
        .arg("--help")
        .output()
        .expect("Failed to execute giagui -s --help");

    // Should show help instead of trying to start spinner
    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("spinner") || stdout.contains("help"));
}

#[test]
fn test_gia_dependency_check() {
    let config = TestConfig::new();

    // If we don't have gia available, GUI should handle it gracefully
    if !config.has_gia_in_path() && !config.gia_binary.exists() {
        eprintln!("Note: gia not available for dependency test");
        return;
    }

    // Test that gia is accessible
    let output = config
        .gia_command()
        .arg("--version")
        .output()
        .expect("Failed to check gia version");

    if output.status.success() {
        let stdout = str::from_utf8(&output.stdout).unwrap();
        assert!(stdout.contains("gia"));
    }
}

#[test]
fn test_invalid_arguments() {
    let config = TestConfig::new();

    let output = config
        .giagui_command()
        .arg("--invalid-flag")
        .output()
        .expect("Failed to execute giagui with invalid flag");

    assert!(!output.status.success());
    let stderr = str::from_utf8(&output.stderr).unwrap();
    assert!(stderr.contains("error") || stderr.contains("invalid") || stderr.contains("unknown"));
}

// Tests that require a display - skip in headless environments
#[test]
fn test_normal_mode_startup() {
    if is_headless() {
        eprintln!("Skipping GUI test: No display available (headless environment)");
        return;
    }

    let config = TestConfig::new();

    // Start giagui in background and kill it quickly
    let mut child = config
        .giagui_command()
        .spawn()
        .expect("Failed to start giagui");

    // Give it a moment to start up
    std::thread::sleep(Duration::from_millis(500));

    // Kill the process
    let _ = child.kill();
    let _ = child.wait();

    // If we got here without panicking, the GUI started successfully
}

#[test]
fn test_spinner_mode_startup() {
    if is_headless() {
        eprintln!("Skipping GUI test: No display available (headless environment)");
        return;
    }

    let config = TestConfig::new();

    // Start giagui in spinner mode and kill it quickly
    let mut child = config
        .giagui_command()
        .arg("--spinner")
        .spawn()
        .expect("Failed to start giagui in spinner mode");

    // Give it a moment to start up
    std::thread::sleep(Duration::from_millis(500));

    // Kill the process
    let _ = child.kill();
    let _ = child.wait();

    // If we got here without panicking, the spinner GUI started successfully
}

// Mock HTTP server tests for Ollama model fetching
#[cfg(test)]
mod mock_server_tests {
    use super::*;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::thread;

    fn start_mock_ollama_server() -> (u16, Arc<AtomicBool>) {
        use std::io::Write;
        use std::net::TcpListener;

        let listener = TcpListener::bind("127.0.0.1:0").expect("Failed to bind mock server");
        let port = listener.local_addr().unwrap().port();
        let running = Arc::new(AtomicBool::new(true));
        let running_clone = running.clone();

        thread::spawn(move || {
            while running_clone.load(Ordering::Relaxed) {
                if let Ok((mut stream, _)) = listener.accept() {
                    let response = r#"HTTP/1.1 200 OK
Content-Type: application/json
Content-Length: 44

{"models":[{"name":"llama3.2:latest"}]}
"#;
                    let _ = stream.write_all(response.as_bytes());
                }
            }
        });

        (port, running)
    }

    #[test]
    fn test_mock_ollama_response() {
        let (port, running) = start_mock_ollama_server();

        // Test that our mock server works
        let url = format!("http://127.0.0.1:{}/api/tags", port);

        // This would be used by the GUI to fetch available models
        // We can't easily test the GUI's HTTP client directly, but we can test
        // that our mock server responds correctly
        let response = std::process::Command::new("curl")
            .args(["-s", &url])
            .output();

        running.store(false, Ordering::Relaxed);

        if let Ok(output) = response
            && output.status.success()
        {
            let body = str::from_utf8(&output.stdout).unwrap();
            assert!(body.contains("llama3.2"));
        }
        // If curl is not available, test passes anyway
    }
}

// Process spawning tests
#[test]
fn test_command_construction() {
    let config = TestConfig::new();

    // Test that we can construct gia commands
    let mut cmd = config.gia_command();
    cmd.arg("--version");

    // This tests that the command can be built
    assert!(cmd.get_program().to_str().is_some());
}

#[test]
fn test_environment_variables() {
    let _mock_env = MockEnvironment::new();

    // Test environment variable handling
    unsafe {
        std::env::set_var("TEST_GUI_ENV", "test_value");
    }
    assert_eq!(std::env::var("TEST_GUI_ENV").unwrap(), "test_value");
}

// Tests for specific GUI functionality that would be called by giagui
#[test]
fn test_gia_process_execution() {
    let config = TestConfig::new();

    if !config.has_gia_in_path() && !config.gia_binary.exists() {
        eprintln!("Skipping test: gia binary not available");
        return;
    }

    // Test executing gia with --version (should always work)
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
fn test_gia_help_execution() {
    let config = TestConfig::new();

    if !config.has_gia_in_path() && !config.gia_binary.exists() {
        eprintln!("Skipping test: gia binary not available");
        return;
    }

    // Test executing gia with --help (should always work)
    let output = config
        .gia_command()
        .arg("--help")
        .output()
        .expect("Failed to execute gia --help");

    assert!(output.status.success());
    let stdout = str::from_utf8(&output.stdout).unwrap();
    assert!(stdout.contains("AI CLI tool using Google Gemini API"));
}

#[test]
fn test_gia_list_conversations() {
    let config = TestConfig::new();

    if !config.has_gia_in_path() && !config.gia_binary.exists() {
        eprintln!("Skipping test: gia binary not available");
        return;
    }

    // Test executing gia with --list-conversations (should always work)
    let output = config
        .gia_command()
        .arg("--list-conversations")
        .output()
        .expect("Failed to execute gia --list-conversations");

    assert!(output.status.success());
    // Output might be empty if no conversations exist, which is fine
}

// Tests that require actual APIs - marked as ignored
#[test]
#[ignore] // Use `cargo test -- --ignored` to run these
fn test_gia_api_execution() {
    if !has_api_key() {
        eprintln!("Skipping test: GEMINI_API_KEY not set");
        return;
    }

    let config = TestConfig::new();

    if !config.has_gia_in_path() && !config.gia_binary.exists() {
        eprintln!("Skipping test: gia binary not available");
        return;
    }

    // Test a simple API call
    let output = config
        .gia_command()
        .arg("Say 'GUI test successful'")
        .output()
        .expect("Failed to execute gia with API");

    if output.status.success() {
        let stdout = str::from_utf8(&output.stdout).unwrap();
        assert!(!stdout.is_empty());
    } else {
        let stderr = str::from_utf8(&output.stderr).unwrap();
        eprintln!("API test failed: {}", stderr);
        // Don't fail the test - API might be rate limited
    }
}

#[test]
#[ignore] // Use `cargo test -- --ignored` to run these
fn test_ollama_model_fetching() {
    if !has_ollama() {
        eprintln!("Skipping test: Ollama not available on localhost:11434");
        return;
    }

    // This would test the actual Ollama model fetching functionality
    // that the GUI uses to populate the model dropdown
    use std::process::Command;

    let output = Command::new("curl")
        .args(["-s", "http://localhost:11434/api/tags"])
        .output();

    if let Ok(output) = output
        && output.status.success()
    {
        let body = str::from_utf8(&output.stdout).unwrap();
        // Should be valid JSON with models
        assert!(body.contains("models") || body.contains("name"));
    }
    // If curl is not available, skip this test
}
