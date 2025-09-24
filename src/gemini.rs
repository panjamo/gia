use crate::api_key::{get_api_key, validate_api_key_format};
use crate::logging::{log_debug, log_error, log_info};
use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};

const GEMINI_API_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
const GEMINI_API_KEY_URL: &str = "https://makersuite.google.com/app/apikey";

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
        let api_key = get_api_key()?;

        if !validate_api_key_format(&api_key) {
            log_error("API key format validation failed, but continuing anyway");
            eprintln!("⚠️  Warning: The API key format seems incorrect.");
            eprintln!("   Expected format: AIzaSy... (39 characters)");
            eprintln!("   If you encounter authentication errors, please check your API key.");
            eprintln!();
        }

        log_info("Initializing Gemini API client");

        Ok(Self {
            client: Client::new(),
            api_key,
            model: "gemini-1.5-flash".to_string(),
        })
    }

    pub async fn generate_content(&self, prompt: &str) -> Result<String> {
        log_debug(&format!(
            "Sending request to Gemini API with prompt length: {}",
            prompt.len()
        ));

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
            let status = response.status();
            let error_text = response
                .text()
                .await
                .unwrap_or_else(|_| "Unknown error".to_string());

            log_error(&format!("Gemini API error ({}): {}", status, error_text));

            // Handle authentication errors specifically
            if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                return self.handle_auth_error(&error_text);
            }

            // Handle quota/billing errors
            if status == StatusCode::TOO_MANY_REQUESTS {
                eprintln!("⚠️  Rate limit exceeded. Please wait and try again.");
                eprintln!("   If this persists, check your API quota at: https://console.cloud.google.com/");
            }

            return Err(anyhow::anyhow!(
                "Gemini API request failed ({}): {}",
                status,
                error_text
            ));
        }

        let gemini_response: GeminiResponse = response
            .json()
            .await
            .context("Failed to parse Gemini API response")?;

        let generated_text = gemini_response
            .candidates
            .first()
            .and_then(|candidate| candidate.content.parts.first())
            .map(|part| part.text.clone())
            .context("No generated content in response")?;

        log_info(&format!(
            "Received response from Gemini API, length: {}",
            generated_text.len()
        ));

        Ok(generated_text)
    }

    fn handle_auth_error(&self, error_text: &str) -> Result<String> {
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
        eprintln!("Error details: {}", error_text);
        eprintln!();
        eprintln!("To fix this:");
        eprintln!("1. Verify your API key at: {}", GEMINI_API_KEY_URL);
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
                    log_error(&format!("Failed to open browser: {}", e));
                    eprintln!(
                        "Could not open browser. Please visit: {}",
                        GEMINI_API_KEY_URL
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
