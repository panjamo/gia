use crate::conversation::TokenUsage;
use crate::logging::{log_debug, log_error, log_info, log_trace, log_warn};
use crate::provider::{AiProvider, AiResponse};
use anyhow::{Context, Result};
use async_trait::async_trait;
use genai::chat::{ChatMessage, ChatRole};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
struct OllamaMessage {
    role: String,
    content: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    images: Option<Vec<String>>,
}

#[derive(Debug, Serialize)]
struct OllamaChatRequest {
    model: String,
    messages: Vec<OllamaMessage>,
    stream: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaUsage {
    #[serde(default)]
    prompt_tokens: Option<u32>,
    #[serde(default)]
    completion_tokens: Option<u32>,
    #[serde(default)]
    total_tokens: Option<u32>,
}

#[derive(Debug, Deserialize)]
struct OllamaChatResponse {
    message: OllamaResponseMessage,
    #[serde(default)]
    usage: Option<OllamaUsage>,
    done: bool,
}

#[derive(Debug, Deserialize)]
struct OllamaResponseMessage {
    content: String,
}

#[derive(Debug)]
pub struct OllamaClient {
    base_url: String,
    model: String,
    client: reqwest::Client,
}

impl OllamaClient {
    pub fn new(model: String) -> Result<Self> {
        let base_url = "http://localhost:11434".to_string();

        log_info(&format!(
            "Initializing Ollama client with model: {} at {}",
            model, base_url
        ));

        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300)) // 5 minute timeout for long responses
            .build()
            .context("Failed to create HTTP client")?;

        Ok(Self {
            base_url,
            model,
            client,
        })
    }

    /// Convert genai ChatMessage to Ollama format
    fn convert_chat_messages(&self, messages: Vec<ChatMessage>) -> Result<Vec<OllamaMessage>> {
        let mut ollama_messages = Vec::new();

        for msg in messages {
            let role = match msg.role {
                ChatRole::User => "user".to_string(),
                ChatRole::Assistant => "assistant".to_string(),
                ChatRole::System => "system".to_string(),
                ChatRole::Tool => {
                    log_warn("Tool role not supported by Ollama, converting to user");
                    "user".to_string()
                }
            };

            match msg.content {
                genai::chat::MessageContent::Text(text) => {
                    ollama_messages.push(OllamaMessage {
                        role,
                        content: text,
                        images: None,
                    });
                }
                genai::chat::MessageContent::Parts(parts) => {
                    let mut text_parts = Vec::new();
                    let mut images = Vec::new();

                    for part in parts {
                        match part {
                            genai::chat::ContentPart::Text(text) => {
                                text_parts.push(text);
                            }
                            genai::chat::ContentPart::Image {
                                content_type,
                                source,
                            } => {
                                if content_type.starts_with("image/") {
                                    // Extract base64 data from ImageSource
                                    match source {
                                        genai::chat::ImageSource::Base64(base64_data) => {
                                            images.push(base64_data.to_string());
                                        }
                                        genai::chat::ImageSource::Url(_url) => {
                                            log_warn("Image URLs are not yet supported for Ollama conversion");
                                        }
                                    }
                                } else {
                                    log_warn(&format!(
                                        "Unsupported inline data type: {}",
                                        content_type
                                    ));
                                }
                            }
                        }
                    }

                    let content = text_parts.join("\n\n");
                    ollama_messages.push(OllamaMessage {
                        role,
                        content,
                        images: if images.is_empty() {
                            None
                        } else {
                            Some(images)
                        },
                    });
                }
                _ => {
                    log_warn(&format!(
                        "Unsupported message content type: {:?}",
                        msg.content
                    ));
                }
            }
        }

        Ok(ollama_messages)
    }

    async fn send_chat_request(&self, messages: Vec<OllamaMessage>) -> Result<AiResponse> {
        let endpoint = format!("{}/api/chat", self.base_url);

        let request_body = OllamaChatRequest {
            model: self.model.clone(),
            messages,
            stream: false,
        };

        log_trace(&format!(
            "Sending request to Ollama API: {:?}",
            request_body
        ));

        let response = self
            .client
            .post(&endpoint)
            .json(&request_body)
            .send()
            .await
            .context("Failed to send request to Ollama API")?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());
            log_error(&format!("Ollama API error: {} - {}", status, error_text));
            return Err(anyhow::anyhow!(
                "Ollama API request failed with status {}: {}",
                status,
                error_text
            ));
        }

        let chat_response: OllamaChatResponse = response
            .json()
            .await
            .context("Failed to parse Ollama response")?;

        log_trace(&format!(
            "Received response from Ollama: {:?}",
            chat_response
        ));

        if !chat_response.done {
            log_warn("Received incomplete response from Ollama");
        }

        let content = chat_response.message.content;

        if content.trim().is_empty() {
            log_error("Generated text is empty");
            return Err(anyhow::anyhow!(
                "No content was generated by the AI. The response was empty or contained only whitespace."
            ));
        }

        log_info(&format!(
            "Received response from Ollama API, length: {}",
            content.len()
        ));

        // Extract usage information if available
        let usage = if let Some(u) = chat_response.usage {
            TokenUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }
        } else {
            TokenUsage::default()
        };

        Ok(AiResponse { content, usage })
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

        let ollama_messages = self
            .convert_chat_messages(chat_messages)
            .context("Failed to convert chat messages to Ollama format")?;

        log_info(&format!(
            "Converted to {} Ollama message(s)",
            ollama_messages.len()
        ));

        self.send_chat_request(ollama_messages).await
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

    #[tokio::test]
    async fn test_ollama_client_creation() {
        let model = "llama3.2".to_string();
        let client = OllamaClient::new(model.clone());
        assert!(client.is_ok());

        let client = client.unwrap();
        assert_eq!(client.model_name(), &model);
        assert_eq!(client.provider_name(), "Ollama");
        assert_eq!(client.base_url, "http://localhost:11434");
    }
}
