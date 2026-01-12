use crate::api_key::validate_api_key_format;
use crate::constants::GEMINI_API_KEY_URL;
use crate::conversation::TokenUsage;
use crate::logging::{log_debug, log_error, log_info, log_trace, log_warn};
use crate::provider::{AiProvider, AiResponse};
use anyhow::{Context, Result};
use async_trait::async_trait;
use genai::Client;
use genai::chat::{ChatMessage, ChatRequest, ChatResponse};
use genai::resolver::{AuthData, AuthResolver};

#[derive(Debug)]
pub struct GeminiClient {
    api_keys: Vec<String>,
    current_key_index: usize,
    model: String,
}

impl GeminiClient {
    pub fn new(model: String, api_keys: Vec<String>, preferred_key_index: usize) -> Result<Self> {
        if api_keys.is_empty() {
            return Err(anyhow::anyhow!("No API keys provided"));
        }

        // Validate API key formats
        for key in &api_keys {
            if !validate_api_key_format(key) {
                log_warn(&format!("API key format validation failed for key: {key}"));
                eprintln!("⚠️  Warning: API key format seems incorrect.");
                eprintln!("   Expected format: AIzaSy... (39 characters)");
            }
        }

        log_info(&format!(
            "Initializing Gemini API client with model: {} and {} API key(s)",
            model,
            api_keys.len()
        ));

        // Use preferred key index if valid, otherwise use 0
        let current_key_index = if preferred_key_index < api_keys.len() {
            log_info(&format!(
                "Using API key index: {} (for caching consistency)",
                preferred_key_index + 1
            ));
            preferred_key_index
        } else {
            log_warn(&format!(
                "Preferred key index {} out of range (have {} keys), using index 0",
                preferred_key_index + 1,
                api_keys.len()
            ));
            0
        };

        Ok(Self {
            api_keys,
            current_key_index,
            model,
        })
    }

    fn next_key_index(&self) -> usize {
        (self.current_key_index + 1) % self.api_keys.len()
    }

    fn handle_auth_error(error_text: &str) -> Result<String> {
        eprintln!();
        eprintln!("🔐 Authentication Error");
        eprintln!("========================");
        eprintln!();
        eprintln!("The Gemini API rejected your request due to authentication issues.");
        eprintln!();
        eprintln!("Common causes:");
        eprintln!("• Invalid API key");
        eprintln!("• API key doesn't have proper permissions");
        eprintln!("• API key is disabled or suspended");
        eprintln!("• Billing not enabled on your Google Cloud project");
        eprintln!();
        eprintln!("Error details: {error_text}");
        eprintln!();
        eprintln!("To fix this:");
        eprintln!("1. Verify your API key at: {GEMINI_API_KEY_URL}");
        eprintln!("2. Check billing is enabled: https://console.cloud.google.com/billing");
        eprintln!(
            "3. Ensure the Generative AI API is enabled: https://console.cloud.google.com/apis/"
        );
        eprintln!();

        // Ask if user wants to open the API key page
        eprintln!("Open API key page in browser? (y/N)");
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_ok() {
            let response = input.trim().to_lowercase();
            if response == "y" || response == "yes" {
                if let Err(e) = webbrowser::open(GEMINI_API_KEY_URL) {
                    log_error(&format!("Failed to open browser: {e}"));
                    eprintln!("Could not open browser. Please visit: {GEMINI_API_KEY_URL}");
                } else {
                    log_info("Opened API key page in browser");
                }
            }
        }

        Err(anyhow::anyhow!(
            "Authentication failed. Please check your API key and billing settings."
        ))
    }

    /// Log chat request structure
    fn log_chat_request_structure(messages: &[ChatMessage]) {
        log_info("=== Chat Request Structure ===");
        log_info(&format!("Total Messages: {}", messages.len()));

        for (i, msg) in messages.iter().enumerate() {
            log_info(&format!("Message {}: {:?}", i + 1, msg.role));
            let parts = msg.content.parts();
            log_info(&format!("  Type: {} part(s)", parts.len()));
            for (j, part) in parts.iter().enumerate() {
                log_trace(&format!("  Part {}: {:?}", j + 1, part));
            }
        }
        log_info("=== End Chat Request Structure ===");
    }

    /// Send chat request with the given messages using specified API key
    async fn try_chat_request_with_messages(
        &self,
        messages: Vec<ChatMessage>,
        api_key: &str,
        key_index: usize,
    ) -> Result<AiResponse> {
        log_info(&format!(
            "Trying API key {}/{} for chat request with {} message(s)",
            key_index + 1,
            self.api_keys.len(),
            messages.len()
        ));

        // Log request structure
        Self::log_chat_request_structure(&messages);

        // Create client with explicit API key using AuthResolver
        let api_key_clone = api_key.to_string();
        let auth_resolver = AuthResolver::from_resolver_fn(move |_model_iden| {
            Ok(Some(AuthData::from_single(api_key_clone.clone())))
        });

        let client = Client::builder().with_auth_resolver(auth_resolver).build();

        // Create the chat request
        let chat_request = ChatRequest::new(messages);
        log_trace("=== Full Chat Request ===");
        log_trace(&format!("Model: {}", self.model));
        log_trace(&format!("Request Debug: {:?}", chat_request));
        log_trace("=== End Full Chat Request ===");

        // Send the request using genai
        let chat_response = match client.exec_chat(&self.model, chat_request, None).await {
            Ok(response) => response,
            Err(e) => {
                // Log the raw error before adding context
                log_debug(&format!("Raw genai error: {}", e));
                log_debug(&format!("Raw genai error debug: {:?}", e));
                return Err(e).context("Failed to send chat request to Gemini API");
            }
        };

        log_trace("=== Full Chat Response ===");
        log_trace(&format!("Content: {:?}", chat_response.content));
        log_trace(&format!(
            "Reasoning Content: {:?}",
            chat_response.reasoning_content
        ));
        log_trace(&format!("Model Iden: {:?}", chat_response.model_iden));
        log_trace(&format!("Usage: {:?}", chat_response.usage));
        log_trace(&format!("Response Debug: {:?}", chat_response));
        log_trace("=== End Full Chat Response ===");

        // Extract usage information if available
        let usage = TokenUsage {
            prompt_tokens: chat_response.usage.prompt_tokens.map(|t| t as u32),
            completion_tokens: chat_response.usage.completion_tokens.map(|t| t as u32),
            total_tokens: chat_response.usage.total_tokens.map(|t| t as u32),
        };

        // Extract the response text
        let generated_text = chat_response
            .first_text()
            .context("Failed to extract text from Gemini response")?;

        // Check if the generated text is empty or just whitespace
        if generated_text.trim().is_empty() {
            log_error("Generated text is empty");
            return Err(anyhow::anyhow!(
                "No content was generated by the AI. The response was empty or contained only whitespace."
            ));
        }

        log_info(&format!(
            "Received response from Gemini API, length: {}",
            generated_text.len()
        ));

        Ok(AiResponse {
            content: generated_text.to_string(),
            usage,
        })
    }
}

#[async_trait]
impl AiProvider for GeminiClient {
    async fn generate_content_with_chat_messages(
        &mut self,
        chat_messages: Vec<ChatMessage>,
    ) -> Result<AiResponse> {
        log_debug(&format!(
            "Sending chat request to Gemini API with {} message(s)",
            chat_messages.len()
        ));

        let starting_key_index = self.current_key_index;
        let total_keys = self.api_keys.len();
        let mut attempts = 0;

        loop {
            let current_key = self.api_keys[self.current_key_index].clone();
            attempts += 1;

            log_info(&format!(
                "Attempt {} with API key {}/{}",
                attempts,
                self.current_key_index + 1,
                total_keys
            ));

            // Try with current API key
            match self
                .try_chat_request_with_messages(
                    chat_messages.clone(),
                    &current_key,
                    self.current_key_index,
                )
                .await
            {
                Ok(response) => {
                    log_info(&format!(
                        "Successfully received response using API key {}/{}",
                        self.current_key_index + 1,
                        total_keys
                    ));
                    return Ok(response);
                }
                Err(e) => {
                    // Get the full error chain using Debug formatting, which includes all causes
                    let error_debug = format!("{:?}", e);
                    let error_debug_lower = error_debug.to_lowercase();

                    // Log error details at debug level
                    log_debug(&format!("Error occurred: {}", e));
                    log_debug(&format!("Full error chain: {}", error_debug));

                    // Check if it's a rate limit or overload error (429 or 503)
                    // Use debug representation to check the full error chain
                    if error_debug.contains("429")
                        || error_debug.contains("Too Many Requests")
                        || error_debug.contains("503")
                        || error_debug_lower.contains("overloaded")
                    {
                        let error_type = if error_debug.contains("503")
                            || error_debug_lower.contains("overloaded")
                        {
                            "Model overloaded (503)"
                        } else {
                            "Rate limit (429)"
                        };

                        log_warn(&format!(
                            "{} on API key {}/{}",
                            error_type,
                            self.current_key_index + 1,
                            total_keys
                        ));

                        // Move to next key
                        self.current_key_index = self.next_key_index();

                        // Check if we've tried all keys
                        if self.current_key_index == starting_key_index {
                            log_error(&format!(
                                "All {} API keys exhausted after {} attempts",
                                total_keys, attempts
                            ));
                            eprintln!();
                            eprintln!("❌ All {} API keys exhausted", total_keys);
                            eprintln!("   All keys have hit rate limits or model overload errors.");
                            eprintln!("   Please try again later or add more API keys.");
                            eprintln!();
                            return Err(anyhow::anyhow!(
                                "All {} API keys exhausted due to rate limits or overload errors",
                                total_keys
                            ));
                        }

                        // User message for fallback
                        eprintln!(
                            "⚠️  {} on API key. Trying next key... ({}/{})",
                            error_type,
                            self.current_key_index + 1,
                            total_keys
                        );
                        log_info(&format!(
                            "Falling back to API key {}/{}",
                            self.current_key_index + 1,
                            total_keys
                        ));

                        // Continue to next iteration with new key
                        continue;
                    } else {
                        // Check if it's an authentication error
                        if error_debug.contains("401")
                            || error_debug.contains("403")
                            || error_debug_lower.contains("authentication")
                            || error_debug_lower.contains("permission")
                        {
                            Self::handle_auth_error(&error_debug)?;
                            return Err(anyhow::anyhow!("Authentication failed"));
                        }
                        // For non-rate-limit errors, return immediately
                        return Err(e);
                    }
                }
            }
        }
    }

    async fn generate_content_with_request(
        &mut self,
        chat_request: ChatRequest,
    ) -> Result<ChatResponse> {
        let starting_key_index = self.current_key_index;
        let total_keys = self.api_keys.len();
        let mut attempts = 0;

        loop {
            let current_key = self.api_keys[self.current_key_index].clone();
            attempts += 1;

            log_info(&format!(
                "Attempt {} with API key {}/{}",
                attempts,
                self.current_key_index + 1,
                total_keys
            ));

            let api_key_clone = current_key.clone();
            let auth_resolver = AuthResolver::from_resolver_fn(move |_model_iden| {
                Ok(Some(AuthData::from_single(api_key_clone.clone())))
            });

            let client = Client::builder().with_auth_resolver(auth_resolver).build();

            match client
                .exec_chat(&self.model, chat_request.clone(), None)
                .await
            {
                Ok(response) => {
                    log_info(&format!(
                        "Successfully received response using API key {}/{}",
                        self.current_key_index + 1,
                        total_keys
                    ));
                    return Ok(response);
                }
                Err(e) => {
                    let error_debug = format!("{:?}", e);
                    let error_debug_lower = error_debug.to_lowercase();

                    if error_debug.contains("429")
                        || error_debug.contains("Too Many Requests")
                        || error_debug.contains("503")
                        || error_debug_lower.contains("overloaded")
                    {
                        log_warn(&format!(
                            "Rate limit/overload on API key {}/{}",
                            self.current_key_index + 1,
                            total_keys
                        ));

                        self.current_key_index = self.next_key_index();

                        if self.current_key_index == starting_key_index {
                            return Err(anyhow::anyhow!("All {} API keys exhausted", total_keys));
                        }
                        continue;
                    }

                    return Err(e).context("Failed to send chat request");
                }
            }
        }
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn provider_name(&self) -> &'static str {
        "Gemini"
    }

    fn current_api_key_index(&self) -> Option<usize> {
        Some(self.current_key_index)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gemini_client_creation() {
        let test_keys = vec![
            "AIzaSyKey1ForTesting123456789012345".to_string(),
            "AIzaSyKey2ForTesting123456789012345".to_string(),
        ];
        let model = "gemini-2.5-flash-lite".to_string();

        let client = GeminiClient::new(model.clone(), test_keys.clone(), 0);
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.model_name(), &model);
        assert_eq!(client.provider_name(), "Gemini");
        assert_eq!(client.api_keys.len(), 2);
    }

    #[tokio::test]
    async fn test_gemini_client_empty_keys() {
        let empty_keys = vec![];
        let model = "gemini-2.5-flash-lite".to_string();

        let result = GeminiClient::new(model, empty_keys, 0);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_single_key_client() {
        let test_keys = vec!["AIzaSyKey1ForTesting123456789012345".to_string()];
        let model = "gemini-2.5-flash-lite".to_string();

        let client = GeminiClient::new(model, test_keys, 0);
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.api_keys.len(), 1);
        assert_eq!(client.current_key_index, 0);
    }

    #[tokio::test]
    async fn test_next_key_index_round_robin() {
        let test_keys = vec![
            "AIzaSyKey1ForTesting123456789012345".to_string(),
            "AIzaSyKey2ForTesting123456789012345".to_string(),
            "AIzaSyKey3ForTesting123456789012345".to_string(),
        ];
        let model = "gemini-2.5-flash-lite".to_string();

        let mut client = GeminiClient::new(model, test_keys, 0).unwrap();
        client.current_key_index = 0;

        // Test round-robin cycling
        assert_eq!(client.next_key_index(), 1);
        client.current_key_index = 1;
        assert_eq!(client.next_key_index(), 2);
        client.current_key_index = 2;
        assert_eq!(client.next_key_index(), 0); // wraps around
    }

    #[tokio::test]
    async fn test_random_starting_key_selection() {
        let test_keys = vec![
            "AIzaSyKey1ForTesting123456789012345".to_string(),
            "AIzaSyKey2ForTesting123456789012345".to_string(),
            "AIzaSyKey3ForTesting123456789012345".to_string(),
        ];
        let model = "gemini-2.5-flash-lite".to_string();

        // Create multiple clients and verify starting index is within valid range
        for _ in 0..10 {
            let client = GeminiClient::new(model.clone(), test_keys.clone(), 0).unwrap();
            assert!(client.current_key_index < test_keys.len());
        }
    }

    #[tokio::test]
    async fn test_multiple_keys_different_count() {
        // Test with 2 keys
        let two_keys = vec![
            "AIzaSyKey1ForTesting123456789012345".to_string(),
            "AIzaSyKey2ForTesting123456789012345".to_string(),
        ];
        let client = GeminiClient::new("gemini-2.5-flash-lite".to_string(), two_keys, 0).unwrap();
        assert_eq!(client.api_keys.len(), 2);

        // Test with 5 keys
        let five_keys = vec![
            "AIzaSyKey1ForTesting123456789012345".to_string(),
            "AIzaSyKey2ForTesting123456789012345".to_string(),
            "AIzaSyKey3ForTesting123456789012345".to_string(),
            "AIzaSyKey4ForTesting123456789012345".to_string(),
            "AIzaSyKey5ForTesting123456789012345".to_string(),
        ];
        let client = GeminiClient::new("gemini-2.5-flash-lite".to_string(), five_keys, 1).unwrap();
        assert_eq!(client.api_keys.len(), 5);
    }

    #[tokio::test]
    async fn test_preferred_key_index() {
        let test_keys = vec![
            "AIzaSyKey1ForTesting123456789012345".to_string(),
            "AIzaSyKey2ForTesting123456789012345".to_string(),
            "AIzaSyKey3ForTesting123456789012345".to_string(),
        ];
        let model = "gemini-2.5-flash-lite".to_string();

        // Test with preferred index 1
        let client = GeminiClient::new(model.clone(), test_keys.clone(), 1).unwrap();
        assert_eq!(client.current_key_index, 1);

        // Test with preferred index 2
        let client = GeminiClient::new(model.clone(), test_keys.clone(), 2).unwrap();
        assert_eq!(client.current_key_index, 2);

        // Test with out-of-range index (should fall back to 0)
        let client = GeminiClient::new(model.clone(), test_keys.clone(), 10).unwrap();
        assert_eq!(client.current_key_index, 0);
    }

    #[tokio::test]
    async fn test_api_key_index_for_caching() {
        // Test that the same API key index is used consistently for caching
        let test_keys = vec![
            "AIzaSyKey1ForTesting123456789012345".to_string(),
            "AIzaSyKey2ForTesting123456789012345".to_string(),
            "AIzaSyKey3ForTesting123456789012345".to_string(),
        ];
        let model = "gemini-2.5-flash-lite".to_string();

        // Simulate creating a new conversation with a specific key
        let initial_key_index = 1;
        let client1 =
            GeminiClient::new(model.clone(), test_keys.clone(), initial_key_index).unwrap();
        assert_eq!(client1.current_key_index, initial_key_index);
        assert_eq!(client1.current_api_key_index(), Some(initial_key_index));

        // Simulate resuming the conversation with the same key index
        let resumed_key_index = initial_key_index;
        let client2 =
            GeminiClient::new(model.clone(), test_keys.clone(), resumed_key_index).unwrap();
        assert_eq!(client2.current_key_index, resumed_key_index);

        // Both clients should use the same key for caching
        assert_eq!(client1.current_key_index, client2.current_key_index);
    }

    #[tokio::test]
    async fn test_current_api_key_index_trait_method() {
        // Test that the trait method correctly returns the current key index
        use crate::provider::AiProvider;

        let test_keys = vec![
            "AIzaSyKey1ForTesting123456789012345".to_string(),
            "AIzaSyKey2ForTesting123456789012345".to_string(),
        ];
        let model = "gemini-2.5-flash-lite".to_string();

        let client = GeminiClient::new(model, test_keys, 1).unwrap();

        // Test the trait method
        let provider: &dyn AiProvider = &client;
        assert_eq!(provider.current_api_key_index(), Some(1));
    }

    #[test]
    fn test_503_error_detection() {
        // Test that 503 errors are correctly identified
        let error_strings = vec![
            "Error 503: Service Unavailable",
            "503 Service temporarily overloaded",
            "The model is overloaded. Please try again later.",
            "Model overloaded, please retry",
        ];

        for error_str in error_strings {
            let contains_503 = error_str.contains("503") || error_str.contains("overloaded");
            assert!(
                contains_503,
                "Failed to detect 503/overload error in: {}",
                error_str
            );
        }
    }

    #[test]
    fn test_429_error_detection() {
        // Test that 429 errors are correctly identified
        let error_strings = vec![
            "Error 429: Too Many Requests",
            "429 Rate limit exceeded",
            "Too Many Requests - rate limit hit",
        ];

        for error_str in error_strings {
            let contains_429 = error_str.contains("429") || error_str.contains("Too Many Requests");
            assert!(contains_429, "Failed to detect 429 error in: {}", error_str);
        }
    }

    #[test]
    fn test_error_type_differentiation() {
        // Test that we can differentiate between 503 and 429 errors
        let error_503 = "Error 503: Service Unavailable";
        let error_429 = "Error 429: Too Many Requests";
        let error_overload = "Model is overloaded";

        // 503 check
        let is_503 = error_503.contains("503") || error_503.contains("overloaded");
        let is_429 = error_503.contains("429") || error_503.contains("Too Many Requests");
        assert!(is_503);
        assert!(!is_429);

        // 429 check
        let is_503 = error_429.contains("503") || error_429.contains("overloaded");
        let is_429 = error_429.contains("429") || error_429.contains("Too Many Requests");
        assert!(!is_503);
        assert!(is_429);

        // Overload check
        let is_503 = error_overload.contains("503") || error_overload.contains("overloaded");
        let is_429 = error_overload.contains("429") || error_overload.contains("Too Many Requests");
        assert!(is_503);
        assert!(!is_429);
    }

    #[tokio::test]
    async fn test_key_cycling_with_multiple_keys() {
        // Test that key index cycles correctly through multiple keys
        let test_keys = vec![
            "AIzaSyKey1ForTesting123456789012345".to_string(),
            "AIzaSyKey2ForTesting123456789012345".to_string(),
            "AIzaSyKey3ForTesting123456789012345".to_string(),
            "AIzaSyKey4ForTesting123456789012345".to_string(),
        ];
        let model = "gemini-2.5-flash-lite".to_string();

        let mut client = GeminiClient::new(model, test_keys, 0).unwrap();
        let starting_index = client.current_key_index;

        // Simulate cycling through all keys
        for expected_index in 1..4 {
            client.current_key_index = client.next_key_index();
            assert_eq!(client.current_key_index, expected_index);
        }

        // Next cycle should wrap to starting index
        client.current_key_index = client.next_key_index();
        assert_eq!(client.current_key_index, starting_index);
    }

    #[test]
    fn test_non_retryable_error_detection() {
        // Test that non-retryable errors are correctly identified
        let non_retryable_errors = vec![
            "Error 401: Unauthorized",
            "403 Forbidden - invalid API key",
            "Authentication failed",
            "Permission denied",
            "Invalid request format",
            "Bad request",
        ];

        for error_str in non_retryable_errors {
            let is_rate_limit =
                error_str.contains("429") || error_str.contains("Too Many Requests");
            let is_overload = error_str.contains("503") || error_str.contains("overloaded");

            assert!(
                !is_rate_limit && !is_overload,
                "Incorrectly identified non-retryable error as retryable: {}",
                error_str
            );
        }
    }
}
