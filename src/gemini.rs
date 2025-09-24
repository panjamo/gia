use crate::api_key::{get_next_api_key, get_random_api_key, validate_api_key_format};
use crate::logging::{log_debug, log_error, log_info, log_warn};
use anyhow::{Context, Result};
use reqwest::{Client, StatusCode};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::time::sleep;

const GEMINI_API_BASE_URL: &str = "https://generativelanguage.googleapis.com/v1beta";
const GEMINI_API_KEY_URL: &str = "https://makersuite.google.com/app/apikey";
const MAX_RETRIES: usize = 3;
const RETRY_DELAY_MS: u64 = 1000;

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
    current_api_key: String,
    model: String,
}

impl GeminiClient {
    pub fn new(model: String) -> Result<Self> {
        let api_key = get_random_api_key()?;

        if !validate_api_key_format(&api_key) {
            log_error("API key format validation failed, but continuing anyway");
            eprintln!("⚠️  Warning: The API key format seems incorrect.");
            eprintln!("   Expected format: AIzaSy... (39 characters)");
            eprintln!("   If you encounter authentication errors, please check your API key.");
            eprintln!();
        }

        log_info(&format!("Initializing Gemini API client with model: {}", model));

        Ok(Self {
            client: Client::new(),
            current_api_key: api_key,
            model,
        })
    }

    pub async fn generate_content(&mut self, prompt: &str) -> Result<String> {
        // Try with current API key first
        match self
            .try_generate_content(prompt, &self.current_api_key.clone())
            .await
        {
            Ok(result) => Ok(result),
            Err(e) => {
                // Check if it's a 429 error and we can try another key
                if e.to_string().contains("429 Too Many Requests") {
                    log_info("Rate limit hit, trying to fallback to another API key");

                    match get_next_api_key(&self.current_api_key) {
                        Ok(next_key) => {
                            log_info("Found alternative API key, retrying request");
                            self.current_api_key = next_key.clone();

                            match self.try_generate_content(prompt, &next_key).await {
                                Ok(result) => {
                                    log_info("Successfully used alternative API key");
                                    Ok(result)
                                }
                                Err(fallback_error) => {
                                    log_error("Alternative API key also failed");
                                    if fallback_error.to_string().contains("429 Too Many Requests")
                                    {
                                        eprintln!(
                                            "⚠️  Rate limit exceeded on all available API keys."
                                        );
                                    }
                                    Err(fallback_error)
                                }
                            }
                        }
                        Err(_) => {
                            log_warn("No alternative API keys available for fallback");
                            eprintln!(
                                "⚠️  Rate limit exceeded and no alternative API keys available."
                            );
                            Err(e)
                        }
                    }
                } else {
                    Err(e)
                }
            }
        }
    }

    async fn try_generate_content(&self, prompt: &str, api_key: &str) -> Result<String> {
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
            GEMINI_API_BASE_URL, self.model, api_key
        );

        // Retry loop for 503 Service Unavailable errors
        for attempt in 1..=MAX_RETRIES {
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

                // Handle authentication errors specifically (don't retry)
                if status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN {
                    return self.handle_auth_error(&error_text);
                }

                // Handle quota/billing errors (don't retry, but allow key fallback)
                if status == StatusCode::TOO_MANY_REQUESTS {
                    log_info("Rate limit exceeded with current API key, will attempt to use alternative API key if available");
                    return Err(anyhow::anyhow!(
                        "Gemini API request failed ({}): {}",
                        status,
                        error_text
                    ));
                }

                // Handle 503 Service Unavailable - retry with exponential backoff
                if status == StatusCode::SERVICE_UNAVAILABLE {
                    if attempt < MAX_RETRIES {
                        let delay_ms = RETRY_DELAY_MS * (attempt as u64);
                        log_warn(&format!(
                            "Service unavailable (attempt {}/{}), retrying in {}ms",
                            attempt, MAX_RETRIES, delay_ms
                        ));
                        eprintln!(
                            "⚠️  Service temporarily unavailable, retrying in {}ms... ({}/{})",
                            delay_ms, attempt, MAX_RETRIES
                        );
                        sleep(Duration::from_millis(delay_ms)).await;
                        continue;
                    } else {
                        log_error("Max retries exceeded for service unavailable error");
                        eprintln!("❌ Service unavailable after {} attempts", MAX_RETRIES);
                    }
                }

                return Err(anyhow::anyhow!(
                    "Gemini API request failed ({}): {}",
                    status,
                    error_text
                ));
            }

            // Success - parse response
            let gemini_response: GeminiResponse = response
                .json()
                .await
                .context("Failed to parse Gemini API response")?;

            // Check if we have any candidates
            if gemini_response.candidates.is_empty() {
                log_error("Gemini API returned no candidates");
                return Err(anyhow::anyhow!(
                    "No content was generated by the AI. The response contained no candidates."
                ));
            }

            // Get the first candidate
            let candidate = gemini_response
                .candidates
                .first()
                .ok_or_else(|| anyhow::anyhow!("No candidates in response"))?;

            // Check if candidate has content parts
            if candidate.content.parts.is_empty() {
                log_error("Candidate has no content parts");
                return Err(anyhow::anyhow!(
                    "No content was generated by the AI. The response candidate contained no content parts."
                ));
            }

            // Get the generated text
            let generated_text = candidate
                .content
                .parts
                .first()
                .map(|part| part.text.clone())
                .ok_or_else(|| anyhow::anyhow!("No text content in response part"))?;

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

            return Ok(generated_text);
        }

        // This should never be reached due to the return statements above
        unreachable!()
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
