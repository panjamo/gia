use anyhow::Result;
use std::env;
use crate::logging::{log_error, log_info, log_warn};

const GEMINI_API_KEY_URL: &str = "https://makersuite.google.com/app/apikey";
const GEMINI_DOCS_URL: &str = "https://ai.google.dev/gemini-api/docs/api-key";

pub fn get_api_key() -> Result<String> {
    match env::var("GEMINI_API_KEY") {
        Ok(key) if !key.trim().is_empty() => {
            log_info("Found GEMINI_API_KEY environment variable");
            Ok(key)
        }
        Ok(_) => {
            log_error("GEMINI_API_KEY environment variable is empty");
            handle_api_key_error()
        }
        Err(_) => {
            log_error("GEMINI_API_KEY environment variable not found");
            handle_api_key_error()
        }
    }
}

fn handle_api_key_error() -> Result<String> {
    eprintln!();
    eprintln!("🔑 API Key Required");
    eprintln!("===================");
    eprintln!();
    eprintln!("The Google Gemini API key is missing or empty.");
    eprintln!();
    eprintln!("To get your free API key:");
    eprintln!("1. Visit: {}", GEMINI_API_KEY_URL);
    eprintln!("2. Sign in with your Google account");
    eprintln!("3. Click 'Create API Key'");
    eprintln!("4. Copy the generated key");
    eprintln!();
    eprintln!("Then set it as an environment variable:");
    eprintln!();
    eprintln!("Windows (Command Prompt):");
    eprintln!("  set GEMINI_API_KEY=your_api_key_here");
    eprintln!();
    eprintln!("Windows (PowerShell):");
    eprintln!("  $env:GEMINI_API_KEY=\"your_api_key_here\"");
    eprintln!();
    eprintln!("Linux/macOS:");
    eprintln!("  export GEMINI_API_KEY=\"your_api_key_here\"");
    eprintln!();
    eprintln!("For permanent setup, add the export line to your shell profile.");
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
        "GEMINI_API_KEY environment variable is required. Visit {} to get your API key.",
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
    if !api_key.chars().all(|c| c.is_alphanumeric() || c == '-' || c == '_') {
        log_warn("API key contains invalid characters");
        return false;
    }
    
    log_info("API key format validation passed");
    true
}

#[cfg(test)]
mod tests {
    use super::*;

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
}