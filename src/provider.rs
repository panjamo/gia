use anyhow::Result;
use async_trait::async_trait;
use std::fmt::Debug;

use crate::cli::ImageSource;

/// Generic AI provider trait for abstraction across different AI services
#[async_trait]
pub trait AiProvider: Debug + Send + Sync {
    /// Generate content from a text prompt
    async fn generate_content(&mut self, prompt: &str) -> Result<String>;

    /// Generate content from a text prompt with optional images
    async fn generate_content_with_images(
        &mut self,
        prompt: &str,
        image_paths: &[String],
    ) -> Result<String>;

    /// Generate content from a text prompt with mixed image sources
    async fn generate_content_with_image_sources(
        &mut self,
        prompt: &str,
        image_sources: &[ImageSource],
    ) -> Result<String>;

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
    /// Model format: "provider::model" or just "model" (defaults to Gemini)
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
            // Future providers can be added here:
            // "openai" => Ok(Box::new(OpenAiClient::new(model_name.to_string(), config.api_keys)?)),
            // "anthropic" => Ok(Box::new(AnthropicClient::new(model_name.to_string(), config.api_keys)?)),
            _ => Err(anyhow::anyhow!(
                "Unsupported provider: {}. Supported providers: gemini",
                provider_name
            )),
        }
    }
}
