use crate::constants::{API_KEY_LENGTH, API_KEY_PREFIX, GEMINI_API_KEY_URL, GEMINI_DOCS_URL};
use crate::logging::{log_info, log_warn};
use anyhow::Result;
use std::env;

pub fn get_api_keys() -> Result<Vec<String>> {
    // First try environment variable - now supports pipe-separated keys
    if let Ok(keys_string) = env::var("GEMINI_API_KEY") {
        if !keys_string.trim().is_empty() {
            // Split by pipe character and filter out empty strings
            let keys: Vec<String> = keys_string
                .split('|')
                .map(|k| k.trim().to_string())
                .filter(|k| !k.is_empty())
                .collect();

            if !keys.is_empty() {
                log_info(&format!(
                    "Found {} API key(s) in GEMINI_API_KEY environment variable",
                    keys.len()
                ));
                return Ok(keys);
            }
        }
    }

    // If we reach here, no valid keys were found
    handle_api_key_error()
}

fn handle_api_key_error() -> Result<Vec<String>> {
    eprintln!();
    eprintln!("🔑 API Keys Required");
    eprintln!("====================");
    eprintln!();
    eprintln!("No Google Gemini API keys found.");
    eprintln!();
    eprintln!("To get your free API keys:");
    eprintln!("1. Visit: {}", GEMINI_API_KEY_URL);
    eprintln!("2. Sign in with your Google account");
    eprintln!("3. Click 'Create API Key'");
    eprintln!("4. Copy the generated key");
    eprintln!();
    eprintln!("Set API key(s) as environment variable:");
    eprintln!();
    eprintln!("Single key:");
    eprintln!("Windows (Command Prompt):");
    eprintln!("  set GEMINI_API_KEY=your_api_key_here");
    eprintln!();
    eprintln!("Windows (PowerShell):");
    eprintln!("  $env:GEMINI_API_KEY=\"your_api_key_here\"");
    eprintln!();
    eprintln!("Multiple keys (pipe-separated for automatic fallback on rate limits):");
    eprintln!("Windows (Command Prompt):");
    eprintln!("  set GEMINI_API_KEY=key1|key2|key3");
    eprintln!();
    eprintln!("Windows (PowerShell):");
    eprintln!("  $env:GEMINI_API_KEY=\"key1|key2|key3\"");
    eprintln!();
    eprintln!("📚 Documentation: {}", GEMINI_DOCS_URL);
    eprintln!();

    // Ask user if they want to open the API key page
    eprintln!("Would you like to open the API key page in your browser? (y/N)");

    let mut input = String::new();
    if std::io::stdin().read_line(&mut input).is_ok() {
        let response = input.trim().to_lowercase();
        if response == "y" || response == "yes" {
            open_browser(GEMINI_API_KEY_URL);
        }
    }

    Err(anyhow::anyhow!(
        "API keys are required. Visit {} to get your API keys.",
        GEMINI_API_KEY_URL
    ))
}

fn open_browser(url: &str) {
    log_info(&format!("Attempting to open browser to: {}", url));

    match webbrowser::open(url) {
        Ok(_) => {
            log_info("Successfully opened browser");
            eprintln!("✅ Opened {} in your default browser", url);
        }
        Err(e) => {
            log_warn(&format!("Failed to open browser: {}", e));
            eprintln!("❌ Could not open browser automatically.");
            eprintln!("Please manually visit: {}", url);
        }
    }
}

pub fn validate_api_key_format(api_key: &str) -> bool {
    // Basic validation for Google API keys
    // They typically start with "AIza" and are 39 characters long
    if api_key.len() != API_KEY_LENGTH {
        log_warn(&format!(
            "API key length is not {} characters (expected for Google API keys)",
            API_KEY_LENGTH
        ));
        return false;
    }

    if !api_key.starts_with(API_KEY_PREFIX) {
        log_warn(&format!(
            "API key does not start with '{}' (expected for Google API keys)",
            API_KEY_PREFIX
        ));
        return false;
    }

    // Check if it contains only valid characters (alphanumeric, dash, underscore)
    if !api_key
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        log_warn("API key contains invalid characters");
        return false;
    }

    log_info("API key format validation passed");
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    #[test]
    fn test_valid_api_key_format() {
        let valid_key = "AIzaSyDummyKeyForTesting123456789012345";
        assert_eq!(valid_key.len(), 39);
        assert!(validate_api_key_format(valid_key));
    }

    #[test]
    fn test_invalid_api_key_length() {
        let short_key = "AIzaShort";
        assert!(!validate_api_key_format(short_key));
    }

    #[test]
    fn test_invalid_api_key_prefix() {
        let wrong_prefix = "WRONG_DummyKeyForTesting123456789012345";
        assert_eq!(wrong_prefix.len(), 39);
        assert!(!validate_api_key_format(wrong_prefix));
    }

    #[test]
    fn test_invalid_characters() {
        let invalid_chars = "AIzaSyDummy@Key#ForTesting1234567890123";
        assert_eq!(invalid_chars.len(), 39);
        assert!(!validate_api_key_format(invalid_chars));
    }

    #[test]
    #[serial]
    fn test_pipe_separated_api_keys() {
        // Clean up any existing environment variable first
        env::remove_var("GEMINI_API_KEY");

        // Set up test environment variable with pipe-separated keys
        let test_keys = "AIzaSyKey1ForTesting123456789012345|AIzaSyKey2ForTesting123456789012345|AIzaSyKey3ForTesting123456789012345";
        env::set_var("GEMINI_API_KEY", test_keys);

        // Test that we can get the keys and they are properly parsed
        let result = get_api_keys();
        assert!(result.is_ok());

        let keys = result.unwrap();
        assert_eq!(keys.len(), 3);
        assert_eq!(keys[0], "AIzaSyKey1ForTesting123456789012345");
        assert_eq!(keys[1], "AIzaSyKey2ForTesting123456789012345");
        assert_eq!(keys[2], "AIzaSyKey3ForTesting123456789012345");

        // Clean up
        env::remove_var("GEMINI_API_KEY");
    }

    #[test]
    #[serial]
    fn test_pipe_separated_with_spaces() {
        // Clean up any existing environment variable first
        env::remove_var("GEMINI_API_KEY");

        // Test with spaces around separators
        let test_keys = "AIzaSyKey1ForTesting123456789012345 | AIzaSyKey2ForTesting123456789012345 | AIzaSyKey3ForTesting123456789012345";
        env::set_var("GEMINI_API_KEY", test_keys);

        let result = get_api_keys();
        assert!(result.is_ok());

        let keys = result.unwrap();
        assert_eq!(keys.len(), 3);
        // Keys should be trimmed
        assert_eq!(keys[0], "AIzaSyKey1ForTesting123456789012345");
        assert_eq!(keys[1], "AIzaSyKey2ForTesting123456789012345");
        assert_eq!(keys[2], "AIzaSyKey3ForTesting123456789012345");

        env::remove_var("GEMINI_API_KEY");
    }

    #[test]
    #[serial]
    fn test_single_api_key_backward_compatibility() {
        // Clean up any existing environment variable first
        env::remove_var("GEMINI_API_KEY");

        // Test that single key still works (backward compatibility)
        let single_key = "AIzaSySingleKeyTesting123456789012345";
        env::set_var("GEMINI_API_KEY", single_key);

        let result = get_api_keys();
        assert!(result.is_ok());

        let keys = result.unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0], single_key);

        env::remove_var("GEMINI_API_KEY");
    }
}
