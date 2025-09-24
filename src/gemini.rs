use crate::api_key::{get_next_api_key, get_random_api_key, validate_api_key_format};
use crate::logging::{log_debug, log_error, log_info, log_warn};
use anyhow::{Context, Result};
use genai::chat::{ChatMessage, ChatRequest};
use genai::Client;
use std::env;

const GEMINI_API_KEY_URL: &str = "https://makersuite.google.com/app/apikey";

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

        log_info(&format!(
            "Initializing Gemini API client with model: {}",
            model
        ));

        // Set the API key in the environment for genai to use
        env::set_var("GEMINI_API_KEY", &api_key);

        let client = Client::default();

        Ok(Self {
            client,
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
                // Check if it's a rate limit error and we can try another key
                if e.to_string().contains("429") || e.to_string().contains("Too Many Requests") {
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
                                    if fallback_error.to_string().contains("429")
                                        || fallback_error.to_string().contains("Too Many Requests")
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
                    // Check if it's an authentication error
                    if e.to_string().contains("401")
                        || e.to_string().contains("403")
                        || e.to_string().contains("authentication")
                        || e.to_string().contains("permission")
                    {
                        return self.handle_auth_error(&e.to_string());
                    }
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

        // Update the API key in environment for this request
        env::set_var("GEMINI_API_KEY", api_key);

        // Create the chat request
        let chat_req = ChatRequest::new(vec![ChatMessage::user(prompt)]);

        // Send the request using genai
        let chat_res = self
            .client
            .exec_chat(&self.model, chat_req, None)
            .await
            .context("Failed to send request to Gemini API")?;

        // Extract the response text
        let generated_text = chat_res
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
