use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::env;
use crate::logging::{log_info, log_debug, log_error};

const GEMINI_API_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";

#[derive(Debug, Serialize)]
struct GeminiRequest {
    contents: Vec<Content>,
    #[serde(rename = "generationConfig")]
    generation_config: GenerationConfig,
}

#[derive(Debug, Serialize)]
struct Content {
    parts: Vec<Part>,
}

#[derive(Debug, Serialize)]
struct Part {
    text: String,
}

#[derive(Debug, Serialize)]
struct GenerationConfig {
    temperature: f32,
    #[serde(rename = "topP")]
    top_p: f32,
    #[serde(rename = "topK")]
    top_k: i32,
    #[serde(rename = "maxOutputTokens")]
    max_output_tokens: i32,
}

#[derive(Debug, Deserialize)]
struct GeminiResponse {
    candidates: Vec<Candidate>,
}

#[derive(Debug, Deserialize)]
struct Candidate {
    content: ResponseContent,
}

#[derive(Debug, Deserialize)]
struct ResponseContent {
    parts: Vec<ResponsePart>,
}

#[derive(Debug, Deserialize)]
struct ResponsePart {
    text: String,
}

pub struct GeminiClient {
    client: Client,
    api_key: String,
    model: String,
}

impl GeminiClient {
    pub fn new() -> Result<Self> {
        let api_key = env::var("GEMINI_API_KEY")
            .context("GEMINI_API_KEY environment variable not set")?;
        
        log_info("Initializing Gemini API client");
        
        Ok(Self {
            client: Client::new(),
            api_key,
            model: "gemini-1.5-flash".to_string(),
        })
    }

    pub async fn generate_content(&self, prompt: &str) -> Result<String> {
        log_debug(&format!("Sending request to Gemini API with prompt length: {}", prompt.len()));
        
        let request = GeminiRequest {
            contents: vec![Content {
                parts: vec![Part {
                    text: prompt.to_string(),
                }],
            }],
            generation_config: GenerationConfig {
                temperature: 0.7,
                top_p: 0.9,
                top_k: 40,
                max_output_tokens: 8192,
            },
        };

        let url = format!(
            "{}/models/{}:generateContent?key={}",
            GEMINI_API_BASE_URL, self.model, self.api_key
        );

        let response = self
            .client
            .post(&url)
            .json(&request)
            .send()
            .await
            .context("Failed to send request to Gemini API")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_else(|_| "Unknown error".to_string());
            log_error(&format!("Gemini API error: {}", error_text));
            return Err(anyhow::anyhow!("Gemini API request failed: {}", error_text));
        }

        let gemini_response: GeminiResponse = response
            .json()
            .await
            .context("Failed to parse Gemini API response")?;

        let generated_text = gemini_response
            .candidates
            .get(0)
            .and_then(|candidate| candidate.content.parts.get(0))
            .map(|part| part.text.clone())
            .context("No generated content in response")?;

        log_info(&format!("Received response from Gemini API, length: {}", generated_text.len()));
        
        Ok(generated_text)
    }
}