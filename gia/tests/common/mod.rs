//! Common utilities for gia CLI integration tests

use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

/// Test configuration for integration tests
pub struct TestConfig {
    pub temp_dir: TempDir,
    pub gia_binary: PathBuf,
}

impl TestConfig {
    /// Create a new test configuration with temporary directory
    pub fn new() -> Self {
        let temp_dir = TempDir::new().expect("Failed to create temp directory");
        let gia_binary = get_gia_binary_path();

        Self {
            temp_dir,
            gia_binary,
        }
    }

    /// Get the path to the temporary directory
    pub fn temp_path(&self) -> &Path {
        self.temp_dir.path()
    }

    /// Create a temporary file with given content and return its path
    pub fn create_temp_file(&self, name: &str, content: &str) -> PathBuf {
        let file_path = self.temp_path().join(name);
        fs::write(&file_path, content).expect("Failed to write temp file");
        file_path
    }

    /// Create a test image file and return its path
    pub fn create_test_image(&self, name: &str) -> PathBuf {
        // Create a minimal PNG file (1x1 pixel, white)
        let png_data = [
            0x89, 0x50, 0x4E, 0x47, 0x0D, 0x0A, 0x1A, 0x0A, // PNG signature
            0x00, 0x00, 0x00, 0x0D, // IHDR chunk length
            0x49, 0x48, 0x44, 0x52, // IHDR chunk type
            0x00, 0x00, 0x00, 0x01, // Width: 1
            0x00, 0x00, 0x00, 0x01, // Height: 1
            0x08, 0x02, 0x00, 0x00,
            0x00, // Bit depth, color type, compression, filter, interlace
            0x90, 0x77, 0x53, 0xDE, // IHDR CRC
            0x00, 0x00, 0x00, 0x0C, // IDAT chunk length
            0x49, 0x44, 0x41, 0x54, // IDAT chunk type
            0x08, 0x99, 0x01, 0x01, 0x00, 0x00, 0x00, 0xFF, 0xFF, 0x00, 0x00, 0x00, 0x02, 0x00,
            0x01, // IDAT data + CRC
            0x00, 0x00, 0x00, 0x00, // IEND chunk length
            0x49, 0x45, 0x4E, 0x44, // IEND chunk type
            0xAE, 0x42, 0x60, 0x82, // IEND CRC
        ];

        let file_path = self.temp_path().join(name);
        fs::write(&file_path, png_data).expect("Failed to write test image");
        file_path
    }

    /// Create a command to run gia with given arguments
    pub fn gia_command(&self) -> Command {
        Command::new(&self.gia_binary)
    }
}

/// Get the path to the gia binary, either built or from PATH
fn get_gia_binary_path() -> PathBuf {
    // First try to use the built binary from target directory
    let manifest_dir = env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| ".".to_string());
    let target_binary = PathBuf::from(manifest_dir.clone())
        .join("target")
        .join("debug")
        .join(if cfg!(windows) { "gia.exe" } else { "gia" });

    if target_binary.exists() {
        return target_binary;
    }

    // Fallback to release binary
    let release_binary = PathBuf::from(manifest_dir)
        .join("target")
        .join("release")
        .join(if cfg!(windows) { "gia.exe" } else { "gia" });

    if release_binary.exists() {
        return release_binary;
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

    /// Remove an environment variable for testing
    pub fn remove_var(&mut self, key: &str) {
        self.original_vars
            .insert(key.to_string(), env::var(key).ok());
        unsafe {
            env::remove_var(key);
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
    use std::time::Duration;

    TcpStream::connect_timeout(
        &"127.0.0.1:11434".parse().unwrap(),
        Duration::from_millis(1000),
    )
    .is_ok()
}

/// Skip test if API key is not available
#[macro_export]
macro_rules! skip_without_api_key {
    () => {
        if !$crate::common::has_api_key() {
            eprintln!("Skipping test: GEMINI_API_KEY not set");
            return;
        }
    };
}

/// Skip test if Ollama is not available
#[macro_export]
macro_rules! skip_without_ollama {
    () => {
        if !$crate::common::has_ollama() {
            eprintln!("Skipping test: Ollama not available on localhost:11434");
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
        assert!(config.gia_binary.file_name().is_some());
    }

    #[test]
    fn test_temp_file_creation() {
        let config = TestConfig::new();
        let file_path = config.create_temp_file("test.txt", "Hello, World!");

        assert!(file_path.exists());
        let content = fs::read_to_string(&file_path).unwrap();
        assert_eq!(content, "Hello, World!");
    }

    #[test]
    fn test_test_image_creation() {
        let config = TestConfig::new();
        let image_path = config.create_test_image("test.png");

        assert!(image_path.exists());
        assert!(image_path.extension().unwrap() == "png");
    }

    #[test]
    fn test_mock_environment() {
        let original_value = env::var("TEST_VAR").ok();

        {
            let mut mock_env = MockEnvironment::new();
            mock_env.set_var("TEST_VAR", "test_value");
            assert_eq!(env::var("TEST_VAR").unwrap(), "test_value");
        }

        // Environment should be restored
        assert_eq!(env::var("TEST_VAR").ok(), original_value);
    }
}
