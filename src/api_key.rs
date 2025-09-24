use crate::logging::{log_debug, log_error, log_info, log_warn};
use anyhow::Result;
use rand::seq::SliceRandom;
use std::env;
use winreg::enums::*;
use winreg::RegKey;

const GEMINI_API_KEY_URL: &str = "https://makersuite.google.com/app/apikey";
const GEMINI_DOCS_URL: &str = "https://ai.google.dev/gemini-api/docs/api-key";
const REGISTRY_PATH: &str = "Software\\GIA";
const REGISTRY_KEY_NAME: &str = "GEMINI_API_KEYS";

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

    // Try to get keys from Windows registry
    match get_api_keys_from_registry() {
        Ok(keys) if !keys.is_empty() => {
            log_info(&format!(
                "Found {} API keys in Windows registry",
                keys.len()
            ));
            Ok(keys)
        }
        Ok(_) => {
            log_error("No API keys found in registry");
            handle_api_key_error()
        }
        Err(e) => {
            log_error(&format!("Failed to read API keys from registry: {}", e));
            handle_api_key_error()
        }
    }
}

pub fn get_random_api_key() -> Result<String> {
    let keys = get_api_keys()?;
    let mut rng = rand::thread_rng();

    keys.choose(&mut rng)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("No API keys available"))
}

pub fn get_next_api_key(current_key: &str) -> Result<String> {
    let keys = get_api_keys()?;

    log_debug(&format!(
        "get_next_api_key: Total keys available: {}",
        keys.len()
    ));
    log_debug(&format!("get_next_api_key: Current key: '{}'", current_key));

    if keys.len() <= 1 {
        log_warn(&format!(
            "get_next_api_key: Only {} keys available, need at least 2",
            keys.len()
        ));
        return Err(anyhow::anyhow!("No alternative API keys available"));
    }

    // Filter out the current key and get a random one from the remaining
    let alternative_keys: Vec<_> = keys
        .iter()
        .filter(|&k| {
            let matches = k.trim() == current_key.trim();
            log_debug(&format!(
                "get_next_api_key: Comparing '{}' with current '{}': {}",
                k.trim(),
                current_key.trim(),
                !matches
            ));
            !matches
        })
        .collect();

    log_debug(&format!(
        "get_next_api_key: Alternative keys found: {}",
        alternative_keys.len()
    ));

    if alternative_keys.is_empty() {
        log_error("get_next_api_key: No alternative keys found after filtering");
        return Err(anyhow::anyhow!("No alternative API keys available"));
    }

    let mut rng = rand::thread_rng();
    let selected_key = alternative_keys
        .choose(&mut rng)
        .map(|&k| k.clone())
        .ok_or_else(|| anyhow::anyhow!("Failed to select alternative API key"))?;

    log_info(&format!(
        "get_next_api_key: Selected alternative key: '{}'",
        selected_key
    ));
    Ok(selected_key)
}

fn get_api_keys_from_registry() -> Result<Vec<String>> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let key = hkcu
        .open_subkey(REGISTRY_PATH)
        .map_err(|e| anyhow::anyhow!("Failed to open registry key {}: {}", REGISTRY_PATH, e))?;

    // Try to read as Vec<String> first
    match key.get_value::<Vec<String>, _>(REGISTRY_KEY_NAME) {
        Ok(values) => {
            log_debug(&format!(
                "get_api_keys_from_registry: Successfully read Vec<String> with {} items",
                values.len()
            ));
            for (i, val) in values.iter().enumerate() {
                log_debug(&format!(
                    "get_api_keys_from_registry: Raw value {}: '{}'",
                    i, val
                ));
            }

            // Filter out empty strings and trim whitespace
            let filtered_keys: Vec<String> = values
                .into_iter()
                .map(|k| k.trim().to_string())
                .filter(|k| !k.is_empty())
                .collect();

            log_debug(&format!(
                "get_api_keys_from_registry: Filtered keys: {} items",
                filtered_keys.len()
            ));
            for (i, key) in filtered_keys.iter().enumerate() {
                log_debug(&format!(
                    "get_api_keys_from_registry: Filtered key {}: '{}'",
                    i, key
                ));
            }

            Ok(filtered_keys)
        }
        Err(e) => {
            log_warn(&format!(
                "Failed to read as Vec<String>, trying as String: {}",
                e
            ));

            // Try to read as a single string and split it
            match key.get_value::<String, _>(REGISTRY_KEY_NAME) {
                Ok(single_value) => {
                    log_debug(&format!(
                        "get_api_keys_from_registry: Successfully read String: '{}'",
                        single_value
                    ));

                    // Split by newlines or semicolons and filter out empty strings
                    let keys: Vec<String> = single_value
                        .lines()
                        .chain(single_value.split(';'))
                        .map(|k| k.trim().to_string())
                        .filter(|k| !k.is_empty())
                        .collect();

                    log_debug(&format!(
                        "get_api_keys_from_registry: Parsed {} keys from string",
                        keys.len()
                    ));
                    Ok(keys)
                }
                Err(string_err) => {
                    log_error(&format!("Failed to read as String either: {}", string_err));
                    Err(anyhow::anyhow!(
                        "Failed to read API keys from registry: {}",
                        string_err
                    ))
                }
            }
        }
    }
}

pub fn set_api_keys_in_registry(keys: Vec<String>) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (key, _) = hkcu
        .create_subkey(REGISTRY_PATH)
        .map_err(|e| anyhow::anyhow!("Failed to create registry key {}: {}", REGISTRY_PATH, e))?;

    key.set_value(REGISTRY_KEY_NAME, &keys).map_err(|e| {
        anyhow::anyhow!("Failed to set registry value {}: {}", REGISTRY_KEY_NAME, e)
    })?;

    log_info(&format!(
        "Successfully stored {} API keys in registry",
        keys.len()
    ));
    Ok(())
}

// Backward compatibility function
pub fn get_api_key() -> Result<String> {
    get_random_api_key()
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
    eprintln!("To store multiple API keys in Windows registry:");
    eprintln!("1. Open Registry Editor (regedit)");
    eprintln!("2. Navigate to: HKEY_CURRENT_USER\\{}", REGISTRY_PATH);
    eprintln!(
        "3. Create a Multi-String Value (REG_MULTI_SZ) named: {}",
        REGISTRY_KEY_NAME
    );
    eprintln!("4. Add each API key on a separate line");
    eprintln!();
    eprintln!("Alternatively, set API key(s) as environment variable:");
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
        "API keys are required. Visit {} to get your API keys or configure them in the Windows registry.",
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
    if api_key.len() != 39 {
        log_warn("API key length is not 39 characters (expected for Google API keys)");
        return false;
    }

    if !api_key.starts_with("AIza") {
        log_warn("API key does not start with 'AIza' (expected for Google API keys)");
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

    #[test]
    #[serial]
    fn test_get_next_api_key() {
        // Clean up any existing environment variable first
        env::remove_var("GEMINI_API_KEY");

        let test_keys = "AIzaSyKey1ForTesting123456789012345|AIzaSyKey2ForTesting123456789012345|AIzaSyKey3ForTesting123456789012345";
        env::set_var("GEMINI_API_KEY", test_keys);

        let current_key = "AIzaSyKey1ForTesting123456789012345";
        let result = get_next_api_key(current_key);
        assert!(result.is_ok());

        let next_key = result.unwrap();
        // Should get a different key
        assert_ne!(next_key, current_key);
        // Should be one of the other keys
        assert!(
            next_key == "AIzaSyKey2ForTesting123456789012345"
                || next_key == "AIzaSyKey3ForTesting123456789012345"
        );

        env::remove_var("GEMINI_API_KEY");
    }

    #[test]
    #[serial]
    fn test_get_next_api_key_single_key_fails() {
        // Clean up any existing environment variable first
        env::remove_var("GEMINI_API_KEY");

        let single_key = "AIzaSySingleKeyTesting123456789012345";
        env::set_var("GEMINI_API_KEY", single_key);

        let result = get_next_api_key(single_key);
        assert!(result.is_err());

        env::remove_var("GEMINI_API_KEY");
    }
}
