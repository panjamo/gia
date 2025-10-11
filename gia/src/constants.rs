/// API key validation constants
pub const API_KEY_LENGTH: usize = 39;
pub const API_KEY_PREFIX: &str = "AIza";

/// Default model constants
pub const DEFAULT_MODEL: &str = "gemini-2.5-flash-lite";

/// Get default model from environment variable or default
pub fn get_default_model() -> String {
    std::env::var("GIA_DEFAULT_MODEL").unwrap_or_else(|_| DEFAULT_MODEL.to_string())
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

        // Set empty environment variable
        unsafe { env::set_var("GIA_DEFAULT_MODEL", "") };

        let result = get_default_model();
        // Empty string should still be returned (env var exists but is empty)
        assert_eq!(result, "");

        // Clean up
        unsafe { env::remove_var("GIA_DEFAULT_MODEL") };
    }
}
