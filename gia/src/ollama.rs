//! Ollama provider implementation for local LLM integration.
//!
//! # Token Usage Limitation
//!
//! The `genai` crate (v0.4) does not currently expose token usage statistics
//! for Ollama responses. All token counts default to zero. This is a known
//! limitation of the underlying library, not this implementation.
//!
//! # Environment Variables
//!
//! - `OLLAMA_BASE_URL`: Base URL for the Ollama server (optional, defaults to http://localhost:11434)
//! - `GIA_DEFAULT_MODEL`: Default model to use (e.g., "ollama::llama3.2", "gemini-2.5-pro")

use crate::conversation::TokenUsage;
use crate::logging::{log_debug, log_info};
use crate::provider::{AiProvider, AiResponse};
use anyhow::{Context, Result};
use async_trait::async_trait;
use genai::Client;
use genai::ServiceTarget;
use genai::chat::{ChatMessage, ChatRequest, MessageContent};
use genai::resolver::{Endpoint, ServiceTargetResolver};
use std::sync::Arc;

#[derive(Debug)]
pub struct OllamaClient {
    model: String,
    client: Client,
}

impl OllamaClient {
    /// Normalize Ollama base URL to end with /v1/ for genai compatibility
    fn normalize_base_url(base_url: String) -> String {
        if base_url.ends_with("/v1/") {
            base_url
        } else if base_url.ends_with("/v1") {
            format!("{}/", base_url)
        } else if base_url.ends_with('/') {
            format!("{}v1/", base_url)
        } else {
            format!("{}/v1/", base_url)
        }
    }

    /// Merge multiple text-only parts into a single text for Ollama compatibility.
    ///
    /// Ollama's OpenAI compatibility layer doesn't properly handle multiple text parts
    /// in a single message. This function detects messages with multiple text-only parts
    /// (no images/audio) and merges them into a single text.
    ///
    /// This preserves Gemini's prompt caching capability while fixing Ollama compatibility.
    fn merge_text_parts_if_needed(message: ChatMessage) -> ChatMessage {
        let parts = message.content.parts();

        // Check if all parts are text-only (no media)
        let all_text = parts.iter().all(|part| {
            matches!(part, genai::chat::ContentPart::Text(_))
        });

        // Only merge if we have multiple text-only parts
        if all_text && parts.len() > 1 {
            log_debug(&format!(
                "Merging {} text parts for Ollama compatibility",
                parts.len()
            ));

            let merged_text = parts
                .iter()
                .filter_map(|part| {
                    if let genai::chat::ContentPart::Text(text) = part {
                        Some(text.as_str())
                    } else {
                        None
                    }
                })
                .collect::<Vec<&str>>()
                .join("\n\n");

            ChatMessage {
                role: message.role,
                content: MessageContent::from_text(merged_text),
                options: message.options,
            }
        } else {
            message
        }
    }

    pub fn new(model: String) -> Result<Self> {
        log_info(&format!("Initializing Ollama client with model: {}", model));

        let client = if let Ok(base_url) = std::env::var("OLLAMA_BASE_URL") {
            log_info(&format!("Using custom Ollama base URL: {}", base_url));

            let url = Self::normalize_base_url(base_url);

            // Create resolver to override endpoint
            let resolver =
                ServiceTargetResolver::from_resolver_fn(move |mut target: ServiceTarget| {
                    target.endpoint = Endpoint::from_owned(Arc::from(url.as_str()));
                    Ok(target)
                });

            Client::builder()
                .with_service_target_resolver(resolver)
                .build()
        } else {
            log_info("Using default Ollama base URL (http://localhost:11434/v1/)");
            Client::default()
        };

        Ok(Self { model, client })
    }
}

#[async_trait]
impl AiProvider for OllamaClient {
    async fn generate_content_with_chat_messages(
        &mut self,
        chat_messages: Vec<ChatMessage>,
    ) -> Result<AiResponse> {
        log_debug(&format!(
            "Sending chat request to Ollama API with {} message(s)",
            chat_messages.len()
        ));

        // Merge multiple text-only parts for Ollama compatibility
        let processed_messages: Vec<ChatMessage> = chat_messages
            .into_iter()
            .map(Self::merge_text_parts_if_needed)
            .collect();

        let chat_req = ChatRequest::new(processed_messages);

        let chat_res = self
            .client
            .exec_chat(&self.model, chat_req, None)
            .await
            .context("Failed to execute Ollama chat request")?;

        let content = chat_res.first_text().unwrap_or("").to_string();

        if content.trim().is_empty() {
            return Err(anyhow::anyhow!(
                "No content was generated by the AI. The response was empty or contained only whitespace."
            ));
        }

        log_info(&format!(
            "Received response from Ollama API, length: {}",
            content.len()
        ));

        // genai doesn't expose token usage for Ollama, use default
        let usage = TokenUsage::default();

        Ok(AiResponse { content, usage })
    }

    fn model_name(&self) -> &str {
        &self.model
    }

    fn provider_name(&self) -> &'static str {
        "Ollama"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use genai::chat::{ChatRole, ContentPart};

    #[test]
    fn test_normalize_base_url() {
        // Already normalized
        assert_eq!(
            OllamaClient::normalize_base_url("http://localhost:11434/v1/".to_string()),
            "http://localhost:11434/v1/"
        );

        // Missing trailing slash
        assert_eq!(
            OllamaClient::normalize_base_url("http://localhost:11434/v1".to_string()),
            "http://localhost:11434/v1/"
        );

        // Missing /v1/
        assert_eq!(
            OllamaClient::normalize_base_url("http://localhost:11434".to_string()),
            "http://localhost:11434/v1/"
        );

        // Has trailing slash but missing v1
        assert_eq!(
            OllamaClient::normalize_base_url("http://localhost:11434/".to_string()),
            "http://localhost:11434/v1/"
        );

        // Different host
        assert_eq!(
            OllamaClient::normalize_base_url("http://192.168.1.100:8000".to_string()),
            "http://192.168.1.100:8000/v1/"
        );
    }

    #[test]
    fn test_merge_text_parts_single_text() {
        // Single text should not be modified
        let message = ChatMessage {
            role: ChatRole::User,
            content: MessageContent::from_text("Hello"),
            options: None,
        };

        let result = OllamaClient::merge_text_parts_if_needed(message);

        assert_eq!(result.content.parts().len(), 1);
        if let ContentPart::Text(text) = &result.content.parts()[0] {
            assert_eq!(text, "Hello");
        } else {
            panic!("Expected Text content");
        }
    }

    #[test]
    fn test_merge_text_parts_multiple_text() {
        // Multiple text parts should be merged
        let parts = vec![
            ContentPart::Text("Part 1".to_string()),
            ContentPart::Text("Part 2".to_string()),
            ContentPart::Text("Part 3".to_string()),
        ];

        let message = ChatMessage {
            role: ChatRole::User,
            content: MessageContent::from_parts(parts),
            options: None,
        };

        let result = OllamaClient::merge_text_parts_if_needed(message);

        // Should be merged into a single text part
        assert_eq!(result.content.parts().len(), 1);
        if let ContentPart::Text(text) = &result.content.parts()[0] {
            assert_eq!(text, "Part 1\n\nPart 2\n\nPart 3");
        } else {
            panic!("Expected merged Text content");
        }
    }

    #[test]
    fn test_merge_text_parts_single_part_in_parts() {
        // Single part in Parts format should not be merged (leave as-is for proper handling)
        let parts = vec![ContentPart::Text("Only one".to_string())];

        let message = ChatMessage {
            role: ChatRole::User,
            content: MessageContent::from_parts(parts),
            options: None,
        };

        let result = OllamaClient::merge_text_parts_if_needed(message);

        // Single part stays as-is (no merge needed)
        assert_eq!(result.content.parts().len(), 1);
        if let ContentPart::Text(text) = &result.content.parts()[0] {
            assert_eq!(text, "Only one");
        } else {
            panic!("Expected Text content");
        }
    }

    #[tokio::test]
    async fn test_ollama_client_creation() {
        let model = "llama3.2".to_string();
        let client = OllamaClient::new(model.clone());
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.model_name(), &model);
        assert_eq!(client.provider_name(), "Ollama");
    }
}
