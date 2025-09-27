use crate::api_key::validate_api_key_format;
use crate::cli::ImageSource;
use crate::clipboard::{convert_image_data_to_base64, read_clipboard_image};
use crate::constants::GEMINI_API_KEY_URL;
use crate::logging::{log_debug, log_error, log_info, log_warn};
use crate::provider::AiProvider;
use anyhow::{Context, Result};
use async_trait::async_trait;
use genai::chat::{ChatMessage, ChatRequest, ContentPart};
use genai::Client;
use rand::prelude::*;
use std::env;

#[derive(Debug)]
pub struct GeminiClient {
    client: Client,
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
                log_warn(&format!(
                    "API key format validation failed for key: {key}"
                ));
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

        // Set the initial API key
        env::set_var("GEMINI_API_KEY", &api_keys[current_key_index]);

        let client = Client::default();

        Ok(Self {
            client,
            api_keys,
            current_key_index,
            model,
        })
    }

    async fn try_generate_content_with_image_sources(
        &self,
        prompt: &str,
        image_sources: &[ImageSource],
        api_key: &str,
    ) -> Result<String> {
        log_debug(&format!(
            "Sending multimodal request to Gemini API with prompt length: {} and {} image source(s)",
            prompt.len(),
            image_sources.len()
        ));

        // Update the API key in environment for this request
        env::set_var("GEMINI_API_KEY", api_key);

        // Create content parts starting with text
        let mut content_parts = vec![ContentPart::from_text(prompt)];

        // Add image parts from various sources
        for image_source in image_sources {
            match image_source {
                ImageSource::File(image_path) => {
                    log_debug(&format!("Adding file image to request: {image_path}"));

                    // Get MIME type for the image
                    let path = std::path::Path::new(image_path);
                    let mime_type = crate::image::get_mime_type(path).with_context(|| {
                        format!("Failed to get MIME type for image: {image_path}")
                    })?;

                    // Read image as base64
                    let base64_data =
                        crate::image::read_image_as_base64(image_path).with_context(|| {
                            format!("Failed to read image as base64: {image_path}")
                        })?;

                    content_parts.push(ContentPart::from_image_base64(mime_type, base64_data));
                }
                ImageSource::Clipboard => {
                    log_debug("Adding clipboard image to request");

                    // Read image from clipboard
                    let image_data =
                        read_clipboard_image().context("Failed to read image from clipboard")?;

                    // Convert to PNG base64
                    let base64_data = convert_image_data_to_base64(&image_data)
                        .context("Failed to convert clipboard image to PNG base64")?;

                    // Use PNG MIME type for clipboard images
                    content_parts.push(ContentPart::from_image_base64(
                        "image/png".to_string(),
                        base64_data,
                    ));
                }
            }
        }

        // Create the chat request with content parts
        let chat_request = ChatRequest::new(vec![ChatMessage::user(content_parts)]);

        // Send the request using genai
        let chat_response = self
            .client
            .exec_chat(&self.model, chat_request, None)
            .await
            .context("Failed to send multimodal request to Gemini API")?;

        // Extract the response text
        let generated_text = chat_response
            .content_text_as_str()
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

        Ok(generated_text.to_string())
    }

    fn try_next_api_key(&mut self) -> Result<String> {
        if self.api_keys.len() <= 1 {
            return Err(anyhow::anyhow!("No alternative API keys available"));
        }

        // Find next available key (simple round-robin)
        self.current_key_index = (self.current_key_index + 1) % self.api_keys.len();
        Ok(self.api_keys[self.current_key_index].clone())
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
                    eprintln!(
                        "Could not open browser. Please visit: {GEMINI_API_KEY_URL}"
                    );
                } else {
                    log_info("Opened API key page in browser");
                }
            }
        }

        Err(anyhow::anyhow!(
            "Authentication failed. Please check your API key and billing settings."
        ))
    }
}

#[async_trait]
impl AiProvider for GeminiClient {
    async fn generate_content_with_image_sources(
        &mut self,
        prompt: &str,
        image_sources: &[ImageSource],
    ) -> Result<String> {
        let current_key = self.api_keys[self.current_key_index].clone();

        // Try with current API key first
        match self
            .try_generate_content_with_image_sources(prompt, image_sources, &current_key)
            .await
        {
            Ok(result) => Ok(result),
            Err(e) => {
                let error_string = e.to_string();

                // Check if it's a rate limit error and we can try another key
                if error_string.contains("429") || error_string.contains("Too Many Requests") {
                    log_info("Rate limit hit, trying to fallback to another API key");

                    if let Ok(next_key) = self.try_next_api_key() {
                        log_info("Found alternative API key, retrying multimodal request with image sources");

                        match self
                            .try_generate_content_with_image_sources(
                                prompt,
                                image_sources,
                                &next_key,
                            )
                            .await
                        {
                            Ok(result) => {
                                log_info("Successfully used alternative API key for multimodal request with image sources");
                                Ok(result)
                            }
                            Err(fallback_error) => {
                                log_error(
                                    "Alternative API key also failed for multimodal request with image sources",
                                );
                                let fallback_error_string = fallback_error.to_string();
                                if fallback_error_string.contains("429")
                                    || fallback_error_string.contains("Too Many Requests")
                                {
                                    eprintln!(
                                        "⚠️  Rate limit exceeded on all available API keys."
                                    );
                                }
                                Err(fallback_error)
                            }
                        }
                    } else {
                        log_warn("No alternative API keys available for fallback");
                        eprintln!(
                            "⚠️  Rate limit exceeded and no alternative API keys available."
                        );
                        Err(e)
                    }
                } else {
                    // Check if it's an authentication error
                    if error_string.contains("401")
                        || error_string.contains("403")
                        || error_string.contains("authentication")
                        || error_string.contains("permission")
                    {
                        return Self::handle_auth_error(&error_string);
                    }
                    Err(e)
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
}
