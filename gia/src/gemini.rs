use crate::api_key::validate_api_key_format;
use crate::constants::GEMINI_API_KEY_URL;
use crate::conversation::TokenUsage;
use crate::logging::{log_debug, log_error, log_info, log_trace, log_warn};
use crate::provider::{AiProvider, AiResponse};
use anyhow::{Context, Result};
use async_trait::async_trait;
use genai::Client;
use genai::chat::{ChatMessage, ChatRequest};
use genai::resolver::{AuthData, AuthResolver};
use rand::prelude::*;

#[derive(Debug)]
pub struct GeminiClient {
    api_keys: Vec<String>,
    current_key_index: usize,
    model: String,
}

impl GeminiClient {
    pub fn new(model: String, api_keys: Vec<String>) -> Result<Self> {
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

        // Start with a random key
        let mut rng = rand::thread_rng();
        let current_key_index = (0..api_keys.len()).choose(&mut rng).unwrap_or(0);

        log_info(&format!(
            "Selected starting API key index: {} (random selection from {} keys)",
            current_key_index + 1,
            api_keys.len()
        ));

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
        let chat_response = client
            .exec_chat(&self.model, chat_request, None)
            .await
            .context("Failed to send chat request to Gemini API")?;

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
                    let error_string = e.to_string();

                    // Check if it's a rate limit error
                    if error_string.contains("429") || error_string.contains("Too Many Requests") {
                        log_warn(&format!(
                            "Rate limit hit on API key {}/{}",
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
                            eprintln!("   All keys have hit rate limits.");
                            eprintln!("   Please try again later or add more API keys.");
                            eprintln!();
                            return Err(anyhow::anyhow!(
                                "All {} API keys exhausted due to rate limits",
                                total_keys
                            ));
                        }

                        // User message for fallback
                        eprintln!(
                            "⚠️  Rate limit hit on API key. Trying next key... ({}/{})",
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
                        if error_string.contains("401")
                            || error_string.contains("403")
                            || error_string.contains("authentication")
                            || error_string.contains("permission")
                        {
                            Self::handle_auth_error(&error_string)?;
                            return Err(anyhow::anyhow!("Authentication failed"));
                        }
                        // For non-rate-limit errors, return immediately
                        return Err(e);
                    }
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

        let client = GeminiClient::new(model.clone(), test_keys.clone());
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

        let result = GeminiClient::new(model, empty_keys);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_single_key_client() {
        let test_keys = vec!["AIzaSyKey1ForTesting123456789012345".to_string()];
        let model = "gemini-2.5-flash-lite".to_string();

        let client = GeminiClient::new(model, test_keys);
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

        let mut client = GeminiClient::new(model, test_keys).unwrap();
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
            let client = GeminiClient::new(model.clone(), test_keys.clone()).unwrap();
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
        let client = GeminiClient::new("gemini-2.5-flash-lite".to_string(), two_keys).unwrap();
        assert_eq!(client.api_keys.len(), 2);

        // Test with 5 keys
        let five_keys = vec![
            "AIzaSyKey1ForTesting123456789012345".to_string(),
            "AIzaSyKey2ForTesting123456789012345".to_string(),
            "AIzaSyKey3ForTesting123456789012345".to_string(),
            "AIzaSyKey4ForTesting123456789012345".to_string(),
            "AIzaSyKey5ForTesting123456789012345".to_string(),
        ];
        let client = GeminiClient::new("gemini-2.5-flash-lite".to_string(), five_keys).unwrap();
        assert_eq!(client.api_keys.len(), 5);
    }
}
