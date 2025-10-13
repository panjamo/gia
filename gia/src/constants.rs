/// API key validation constants
pub const API_KEY_LENGTH: usize = 39;
pub const API_KEY_PREFIX: &str = "AIza";

/// Default model constants
pub const DEFAULT_MODEL: &str = "gemini-2.5-flash-lite";

/// Get default model from environment variable or default
/// Priority: OLLAMA_MODEL (with ollama:: prefix) > GIA_DEFAULT_MODEL > DEFAULT_MODEL
/// Note: OLLAMA_MODEL takes precedence to allow easy switching to local Ollama without Gemini API keys
pub fn get_default_model() -> String {
    // First check OLLAMA_MODEL - if set, always use Ollama (no Gemini)
    if let Ok(model) = std::env::var("OLLAMA_MODEL") {
        return format!("ollama::{}", model);
    }

    // Then check GIA_DEFAULT_MODEL - for Gemini model selection
    if let Ok(model) = std::env::var("GIA_DEFAULT_MODEL") {
        return model;
    }

    // Fall back to default Gemini model
    DEFAULT_MODEL.to_string()
}

/// Conversation management constants
pub const DEFAULT_CONTEXT_WINDOW_LIMIT: usize = 8000;

/// Get context window limit from environment variable or default
pub fn get_context_window_limit() -> usize {
    std::env::var("CONTEXT_WINDOW_LIMIT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_CONTEXT_WINDOW_LIMIT)
}
pub const CONVERSATION_TRUNCATION_KEEP_MESSAGES: usize = 20;

/// URLs for user guidance
pub const GEMINI_API_KEY_URL: &str = "https://makersuite.google.com/app/apikey";
pub const GEMINI_DOCS_URL: &str = "https://ai.google.dev/gemini-api/docs/api-key";

/// Supported media file extensions
pub const MEDIA_EXTENSIONS: &[&str] = &[
    "jpg", "jpeg", "png", "webp", "heic", "pdf", "ogg", "opus", "mp3", "m4a", "mp4",
];

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;

    #[test]
    #[serial]
    fn test_default_model_without_env_var() {
        // Clean up any existing environment variable first
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };

        let result = get_default_model();
        assert_eq!(result, DEFAULT_MODEL);

        // Clean up
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };
    }

    #[test]
    #[serial]
    fn test_default_model_with_env_var() {
        // Clean up any existing environment variable first
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };

        // Set a custom model via environment variable
        let custom_model = "gemini-2.5-pro";
        unsafe { env::set_var("GIA_DEFAULT_MODEL", custom_model) };

        let result = get_default_model();
        assert_eq!(result, custom_model);

        // Clean up
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };
    }

    #[test]
    #[serial]
    fn test_default_model_with_ollama_format() {
        // Clean up any existing environment variable first
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };

        // Test with Ollama format
        let ollama_model = "ollama::llama3.2";
        unsafe { env::set_var("GIA_DEFAULT_MODEL", ollama_model) };

        let result = get_default_model();
        assert_eq!(result, ollama_model);

        // Clean up
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };
    }

    #[test]
    #[serial]
    fn test_default_model_with_empty_env_var() {
        // Clean up any existing environment variable first
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };
        unsafe { env::remove_var("OLLAMA_MODEL") };

        // Set empty environment variable
        unsafe { env::set_var("GIA_DEFAULT_MODEL", "") };

        let result = get_default_model();
        // Empty string should still be returned (env var exists but is empty)
        assert_eq!(result, "");

        // Clean up
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };
    }

    #[test]
    #[serial]
    fn test_default_model_ollama_model_priority() {
        // Clean up any existing environment variables first
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };
        unsafe { env::remove_var("OLLAMA_MODEL") };

        // Test with only OLLAMA_MODEL set
        unsafe { env::set_var("OLLAMA_MODEL", "llama3.2") };
        let result = get_default_model();
        assert_eq!(result, "ollama::llama3.2");

        // Clean up
        unsafe { env::remove_var("OLLAMA_MODEL") };
    }

    #[test]
    #[serial]
    fn test_default_model_ollama_over_gia_default() {
        // Clean up any existing environment variables first
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };
        unsafe { env::remove_var("OLLAMA_MODEL") };

        // Set both - OLLAMA_MODEL should take priority
        unsafe { env::set_var("OLLAMA_MODEL", "llama3.2") };
        unsafe { env::set_var("GIA_DEFAULT_MODEL", "gemini-2.5-pro") };

        let result = get_default_model();
        // OLLAMA_MODEL should win
        assert_eq!(result, "ollama::llama3.2");

        // Clean up
        unsafe { env::remove_var("OLLAMA_MODEL") };
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };
    }

    #[test]
    #[serial]
    fn test_default_model_gia_default_when_no_ollama() {
        // Clean up any existing environment variables first
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };
        unsafe { env::remove_var("OLLAMA_MODEL") };

        // Set only GIA_DEFAULT_MODEL
        unsafe { env::set_var("GIA_DEFAULT_MODEL", "gemini-2.5-pro") };

        let result = get_default_model();
        // GIA_DEFAULT_MODEL should be used
        assert_eq!(result, "gemini-2.5-pro");

        // Clean up
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };
    }
}
