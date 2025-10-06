/// API key validation constants
pub const API_KEY_LENGTH: usize = 39;
pub const API_KEY_PREFIX: &str = "AIza";

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
