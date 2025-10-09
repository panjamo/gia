use anyhow::Result;
use async_trait::async_trait;
use genai::chat::ChatMessage;
use std::fmt::Debug;

use crate::conversation::TokenUsage;

/// Response from AI provider with content and usage information
#[derive(Debug)]
pub struct AiResponse {
    pub content: String,
    pub usage: TokenUsage,
}

/// Generic AI provider trait for abstraction across different AI services
#[async_trait]
pub trait AiProvider: Debug + Send + Sync {
    /// Generate content from chat messages with usage information
    async fn generate_content_with_chat_messages(
        &mut self,
        chat_messages: Vec<ChatMessage>,
    ) -> Result<AiResponse>;

    /// Get the model name being used
    fn model_name(&self) -> &str;

    /// Get provider-specific information (e.g., "Gemini", "OpenAI", etc.)
    fn provider_name(&self) -> &str;
}

/// Configuration for creating AI providers
#[derive(Debug, Clone)]
pub struct ProviderConfig {
    pub model: String,
    pub api_keys: Vec<String>,
}

/// Factory for creating AI providers
pub struct ProviderFactory;

impl ProviderFactory {
    /// Create a provider based on the model string
    /// Model format: "`provider::model`" or just "model" (defaults to Gemini)
    pub fn create_provider(config: ProviderConfig) -> Result<Box<dyn AiProvider>> {
        let (provider_name, model_name) = if config.model.contains("::") {
            let parts: Vec<&str> = config.model.splitn(2, "::").collect();
            (parts[0], parts[1])
        } else {
            // Default to Gemini for backward compatibility
            ("gemini", config.model.as_str())
        };

        match provider_name.to_lowercase().as_str() {
            "gemini" => {
                let client =
                    crate::gemini::GeminiClient::new(model_name.to_string(), config.api_keys)?;
                Ok(Box::new(client))
            }
            "ollama" => {
                let client = crate::ollama::OllamaClient::new(model_name.to_string())?;
                Ok(Box::new(client))
            }
            // Future providers can be added here:
            // "openai" => Ok(Box::new(OpenAiClient::new(model_name.to_string(), config.api_keys)?)),
            // "anthropic" => Ok(Box::new(AnthropicClient::new(model_name.to_string(), config.api_keys)?)),
            _ => Err(anyhow::anyhow!(
                "Unsupported provider: {provider_name}. Supported providers: gemini, ollama"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_ollama_provider() {
        let config = ProviderConfig {
            model: "ollama::llama3.2".to_string(),
            api_keys: Vec::new(),
        };
        let result = ProviderFactory::create_provider(config);
        assert!(result.is_ok());
        assert_eq!(result.unwrap().provider_name(), "Ollama");
    }

    #[test]
    fn test_unsupported_provider() {
        let config = ProviderConfig {
            model: "unknown::model".to_string(),
            api_keys: Vec::new(),
        };
        let result = ProviderFactory::create_provider(config);
        assert!(result.is_err());
    }
}
