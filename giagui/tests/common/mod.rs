//! Common utilities for giagui integration tests

use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;
use tempfile::TempDir;

/// Test configuration for GUI integration tests
pub struct TestConfig {
    pub temp_dir: TempDir,
    pub giagui_binary: PathBuf,
    pub gia_binary: PathBuf,
}

impl TestConfig {
    /// Create a new test configuration with temporary directory
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let giagui_binary = get_giagui_binary_path();
        let gia_binary = get_gia_binary_path();

        Self {
            temp_dir,
            giagui_binary,
            gia_binary,
        }
    }

    /// Get the path to the temporary directory
    pub fn temp_path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Create a command to run giagui with given arguments
    pub fn giagui_command(&self) -> Command {
        Command::new(&self.giagui_binary)
    }

    /// Create a command to run gia (for testing GUI dependencies)
    pub fn gia_command(&self) -> Command {
        Command::new(&self.gia_binary)
    }

    /// Check if gia binary is available in PATH
    pub fn has_gia_in_path(&self) -> bool {
        Command::new("gia")
            .arg("--version")
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

/// Get the path to the giagui binary, either built or from PATH
fn get_giagui_binary_path() -> PathBuf {
    // First try to use the built binary from target directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let target_binary = PathBuf::from(manifest_dir.clone())
        .join("target")
        .join("debug")
        .join(if cfg!(windows) {
            "giagui.exe"
        } else {
            "giagui"
        });

    if target_binary.exists() {
        return target_binary;
    }

    // Fallback to release binary
    let release_binary = PathBuf::from(manifest_dir)
        .join("target")
        .join("release")
        .join(if cfg!(windows) {
            "giagui.exe"
        } else {
            "giagui"
        });

    if release_binary.exists() {
        return release_binary;
    }

    // Fallback to system PATH
    PathBuf::from(if cfg!(windows) {
        "giagui.exe"
    } else {
        "giagui"
    })
}

/// Get the path to the gia binary (for dependency testing)
fn get_gia_binary_path() -> PathBuf {
    // First try to use the built binary from workspace target directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let workspace_target = PathBuf::from(manifest_dir.clone())
        .parent() // Go up from giagui/ to workspace root
        .unwrap_or_else(|| Path::new("."))
        .join("target")
        .join("debug")
        .join(if cfg!(windows) { "gia.exe" } else { "gia" });

    if workspace_target.exists() {
        return workspace_target;
    }

    // Fallback to release binary
    let workspace_release = PathBuf::from(manifest_dir)
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("target")
        .join("release")
        .join(if cfg!(windows) { "gia.exe" } else { "gia" });

    if workspace_release.exists() {
        return workspace_release;
    }

    // Fallback to system PATH
    PathBuf::from(if cfg!(windows) { "gia.exe" } else { "gia" })
}

/// Mock environment for testing
pub struct MockEnvironment {
    original_vars: std::collections::HashMap<String, Option<String>>,
}

impl MockEnvironment {
    /// Create a new mock environment
    pub fn new() -> Self {
        Self {
            original_vars: std::collections::HashMap::new(),
        }
    }

    /// Set an environment variable for testing
    pub fn set_var(&mut self, key: &str, value: &str) {
        // Store original value for restoration
        self.original_vars
            .insert(key.to_string(), env::var(key).ok());
        unsafe {
            env::set_var(key, value);
        }
    }
}

impl Drop for MockEnvironment {
    fn drop(&mut self) {
        // Restore original environment variables
        for (key, original_value) in &self.original_vars {
            match original_value {
                Some(value) => unsafe { env::set_var(key, value) },
                None => unsafe { env::remove_var(key) },
            }
        }
    }
}

/// Check if we have a valid API key for testing
pub fn has_api_key() -> bool {
    env::var("GEMINI_API_KEY").is_ok()
}

/// Check if Ollama is available for testing
pub fn has_ollama() -> bool {
    // Try to connect to localhost:11434
    use std::net::TcpStream;

    TcpStream::connect_timeout(
        &"127.0.0.1:11434".parse().unwrap(),
        Duration::from_millis(1000),
    )
    .is_ok()
}

/// Check if we're running in a headless environment (no display)
pub fn is_headless() -> bool {
    // Check common environment variables that indicate a headless environment
    env::var("DISPLAY").is_err() && env::var("WAYLAND_DISPLAY").is_err() && !cfg!(windows) // Windows doesn't need DISPLAY
}

/// Skip test if running in headless environment
#[macro_export]
macro_rules! skip_if_headless {
    () => {
        if $crate::common::is_headless() {
            eprintln!("Skipping GUI test: No display available (headless environment)");
            return;
        }
    };
}

/// Skip test if gia is not available
#[macro_export]
macro_rules! skip_without_gia {
    ($config:expr) => {
        if !$config.has_gia_in_path() && !$config.gia_binary.exists() {
            eprintln!("Skipping test: gia binary not available");
            return;
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_creation() {
        let config = TestConfig::new();
        assert!(config.temp_path().exists());
        assert!(config.giagui_binary.file_name().is_some());
        assert!(config.gia_binary.file_name().is_some());
    }

    #[test]
    fn test_binary_paths() {
        let giagui_path = get_giagui_binary_path();
        let gia_path = get_gia_binary_path();

        // Should at least have valid file names
        assert!(giagui_path.file_name().is_some());
        assert!(gia_path.file_name().is_some());

        // Should have correct extensions on Windows
        if cfg!(windows) {
            assert!(giagui_path.extension().is_some_and(|ext| ext == "exe"));
            assert!(gia_path.extension().is_some_and(|ext| ext == "exe"));
        }
    }

    #[test]
    fn test_mock_environment() {
        let original_value = env::var("TEST_GUI_VAR").ok();

        {
            let mut mock_env = MockEnvironment::new();
            mock_env.set_var("TEST_GUI_VAR", "gui_test_value");
            assert_eq!(env::var("TEST_GUI_VAR").unwrap(), "gui_test_value");
        }

        // Environment should be restored
        assert_eq!(env::var("TEST_GUI_VAR").ok(), original_value);
    }

    #[test]
    fn test_headless_detection() {
        // This test will vary by environment, just ensure it doesn't panic
        let _is_headless = is_headless();
    }
}
