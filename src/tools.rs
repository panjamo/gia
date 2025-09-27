use anyhow::{Context, Result};
use regex::Regex;
use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use url::Url;

use crate::logging::{log_debug, log_error, log_info};

// Function calling structures for Gemini API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    #[serde(rename = "functionDeclarations")]
    pub function_declarations: Vec<FunctionDeclaration>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionDeclaration {
    pub name: String,
    pub description: String,
    pub parameters: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionCall {
    pub name: String,
    pub args: HashMap<String, Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FunctionResponse {
    pub name: String,
    pub response: Value,
}

// Tool implementations
#[derive(Debug)]
pub struct WebTools {
    client: Client,
}

impl WebTools {
    pub fn new() -> Self {
        Self {
            client: Client::new(),
        }
    }

    pub fn create_tools() -> Vec<Tool> {
        vec![Tool {
            function_declarations: vec![
                FunctionDeclaration {
                    name: "web_search".to_string(),
                    description: "Search the web for current information about a topic using DuckDuckGo".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "query": {
                                "type": "string",
                                "description": "The search query to find information about"
                            }
                        },
                        "required": ["query"]
                    }),
                },
                FunctionDeclaration {
                    name: "read_webpage".to_string(),
                    description: "Read and extract the main content from a specific webpage URL".to_string(),
                    parameters: json!({
                        "type": "object",
                        "properties": {
                            "url": {
                                "type": "string",
                                "description": "The URL of the webpage to read and extract content from"
                            }
                        },
                        "required": ["url"]
                    }),
                },
            ],
        }]
    }

    pub async fn execute_function_call(&self, call: &FunctionCall) -> Result<FunctionResponse> {
        match call.name.as_str() {
            "web_search" => {
                let query = call
                    .args
                    .get("query")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing or invalid query parameter"))?;

                let results = self.web_search(query).await?;
                Ok(FunctionResponse {
                    name: "web_search".to_string(),
                    response: json!({
                        "query": query,
                        "results": results
                    }),
                })
            }
            "read_webpage" => {
                let url = call
                    .args
                    .get("url")
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| anyhow::anyhow!("Missing or invalid url parameter"))?;

                let content = self.read_webpage(url).await?;
                Ok(FunctionResponse {
                    name: "read_webpage".to_string(),
                    response: json!({
                        "url": url,
                        "content": content
                    }),
                })
            }
            _ => Err(anyhow::anyhow!("Unknown function: {}", call.name)),
        }
    }

    async fn web_search(&self, query: &str) -> Result<Vec<Value>> {
        log_info(&format!("Performing web search for: {}", query));

        // Try DuckDuckGo instant answer API first
        if let Ok(instant_results) = self.duckduckgo_instant_answer(query).await {
            if !instant_results.is_empty() {
                log_info("Found DuckDuckGo instant answer results");
                return Ok(instant_results);
            }
        }

        // Fallback to DuckDuckGo HTML search
        self.duckduckgo_search(query).await
    }

    async fn duckduckgo_instant_answer(&self, query: &str) -> Result<Vec<Value>> {
        let url = format!(
            "https://api.duckduckgo.com/?q={}&format=json&no_html=1&skip_disambig=1",
            urlencoding::encode(query)
        );

        log_debug(&format!("Requesting DuckDuckGo instant answer: {}", url));

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "GIA/1.0 (Web Search Tool)")
            .send()
            .await
            .context("Failed to send request to DuckDuckGo API")?;

        let json: Value = response
            .json()
            .await
            .context("Failed to parse DuckDuckGo API response")?;

        let mut results = Vec::new();

        // Check for abstract (direct answer)
        if let Some(abstract_text) = json.get("Abstract").and_then(|v| v.as_str()) {
            if !abstract_text.is_empty() {
                results.push(json!({
                    "title": json.get("Heading").and_then(|v| v.as_str()).unwrap_or("Answer"),
                    "snippet": abstract_text,
                    "url": json.get("AbstractURL").and_then(|v| v.as_str()).unwrap_or(""),
                    "source": "DuckDuckGo Instant Answer"
                }));
            }
        }

        // Check for definition
        if let Some(definition) = json.get("Definition").and_then(|v| v.as_str()) {
            if !definition.is_empty() {
                results.push(json!({
                    "title": "Definition",
                    "snippet": definition,
                    "url": json.get("DefinitionURL").and_then(|v| v.as_str()).unwrap_or(""),
                    "source": "DuckDuckGo Definition"
                }));
            }
        }

        // Check for related topics
        if let Some(topics) = json.get("RelatedTopics").and_then(|v| v.as_array()) {
            for topic in topics.iter().take(3) {
                if let Some(text) = topic.get("Text").and_then(|v| v.as_str()) {
                    if !text.is_empty() {
                        results.push(json!({
                            "title": "Related",
                            "snippet": text,
                            "url": topic.get("FirstURL").and_then(|v| v.as_str()).unwrap_or(""),
                            "source": "DuckDuckGo Related"
                        }));
                    }
                }
            }
        }

        Ok(results)
    }

    async fn duckduckgo_search(&self, query: &str) -> Result<Vec<Value>> {
        log_debug("Performing DuckDuckGo HTML search");

        let url = format!("https://html.duckduckgo.com/html/?q={}", urlencoding::encode(query));

        let response = self
            .client
            .get(&url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await
            .context("Failed to send request to DuckDuckGo")?;

        let html = response
            .text()
            .await
            .context("Failed to get HTML from DuckDuckGo")?;

        let document = Html::parse_document(&html);
        let result_selector = Selector::parse("div.result").unwrap();
        let title_selector = Selector::parse("a.result__a").unwrap();
        let snippet_selector = Selector::parse("a.result__snippet").unwrap();

        let mut results = Vec::new();

        for result in document.select(&result_selector).take(5) {
            let title = result
                .select(&title_selector)
                .next()
                .map(|el| el.inner_html())
                .unwrap_or_else(|| "No title".to_string());

            let snippet = result
                .select(&snippet_selector)
                .next()
                .map(|el| el.inner_html())
                .unwrap_or_else(|| "No description".to_string());

            let url = result
                .select(&title_selector)
                .next()
                .and_then(|el| el.value().attr("href"))
                .unwrap_or("")
                .to_string();

            // Clean HTML from title and snippet
            let title = self.clean_html(&title);
            let snippet = self.clean_html(&snippet);

            if !title.is_empty() && !snippet.is_empty() {
                results.push(json!({
                    "title": title,
                    "snippet": snippet,
                    "url": url,
                    "source": "DuckDuckGo Search"
                }));
            }
        }

        if results.is_empty() {
            log_error("No search results found");
            return Err(anyhow::anyhow!("No search results found for query: {}", query));
        }

        log_info(&format!("Found {} search results", results.len()));
        Ok(results)
    }

    async fn read_webpage(&self, url: &str) -> Result<String> {
        log_info(&format!("Reading webpage: {}", url));

        // Normalize URL - add protocol if missing
        let normalized_url = if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else {
            format!("https://{}", url)
        };

        log_debug(&format!("Normalized URL: {}", normalized_url));

        // Validate URL
        let _parsed_url = Url::parse(&normalized_url).context("Invalid URL format")?;

        let response = self
            .client
            .get(&normalized_url)
            .header("User-Agent", "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36")
            .send()
            .await
            .context("Failed to fetch webpage")?;

        if !response.status().is_success() {
            return Err(anyhow::anyhow!(
                "HTTP error {}: {}",
                response.status(),
                response.status().canonical_reason().unwrap_or("Unknown error")
            ));
        }

        let html = response.text().await.context("Failed to get HTML content")?;
        let document = Html::parse_document(&html);

        // Try different content extraction strategies
        let content = self.extract_main_content(&document, &normalized_url)?;

        // Limit content length to avoid token overflow
        let max_length = 4000;
        let final_content = if content.len() > max_length {
            let truncated = &content[..max_length];
            format!("{}...\n\n[Content truncated due to length]", truncated)
        } else {
            content
        };

        log_info(&format!("Extracted {} characters from webpage", final_content.len()));
        Ok(final_content)
    }

    fn extract_main_content(&self, document: &Html, url: &str) -> Result<String> {
        // Site-specific selectors for better content extraction
        let selectors = if url.contains("github.com") {
            vec!["article", "div.markdown-body", "div.entry-content"]
        } else if url.contains("stackoverflow.com") {
            vec!["div.question", "div.answer"]
        } else if url.contains("reddit.com") {
            vec!["div.usertext-body", "div.md"]
        } else if url.contains("wikipedia.org") {
            vec!["div.mw-parser-output", "div#bodyContent"]
        } else {
            vec![
                "main",
                "article",
                "div.content",
                "div.entry-content",
                "div.post-content",
                "div.article-body",
                "div.text",
                "section.content",
                "div.container",
            ]
        };

        // Try selectors in order of preference
        for selector_str in selectors {
            if let Ok(selector) = Selector::parse(selector_str) {
                if let Some(content_elem) = document.select(&selector).next() {
                    let content = self.extract_text_content(content_elem);
                    if content.len() > 100 {
                        // Ensure we got substantial content
                        return Ok(content);
                    }
                }
            }
        }

        // Fallback to body content
        if let Ok(body_selector) = Selector::parse("body") {
            if let Some(body) = document.select(&body_selector).next() {
                let content = self.extract_text_content(body);
                if !content.is_empty() {
                    return Ok(content);
                }
            }
        }

        Err(anyhow::anyhow!("Could not extract meaningful content from webpage"))
    }

    fn extract_text_content(&self, element: scraper::ElementRef) -> String {
        let mut text = element.text().collect::<Vec<_>>().join(" ");

        // Clean up whitespace
        let re = Regex::new(r"\s+").unwrap();
        text = re.replace_all(&text, " ").trim().to_string();

        // Remove common unwanted sections
        let unwanted_patterns = [
            r"(?i)cookie.{0,50}accept",
            r"(?i)privacy.{0,50}policy",
            r"(?i)terms.{0,50}service",
            r"(?i)subscribe.{0,50}newsletter",
            r"(?i)follow.{0,50}us",
            r"(?i)share.{0,50}(facebook|twitter|linkedin)",
        ];

        for pattern in &unwanted_patterns {
            if let Ok(re) = Regex::new(pattern) {
                text = re.replace_all(&text, "").to_string();
            }
        }

        text.trim().to_string()
    }

    fn clean_html(&self, html: &str) -> String {
        let document = Html::parse_fragment(html);
        document.root_element().text().collect::<String>()
    }
}

impl Default for WebTools {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_tools() {
        let tools = WebTools::create_tools();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0].function_declarations.len(), 2);
        
        let function_names: Vec<&String> = tools[0]
            .function_declarations
            .iter()
            .map(|f| &f.name)
            .collect();
        
        assert!(function_names.contains(&&"web_search".to_string()));
        assert!(function_names.contains(&&"read_webpage".to_string()));
    }

    #[test]
    fn test_clean_html() {
        let tools = WebTools::new();
        let html = "<b>Bold text</b> and <i>italic text</i>";
        let cleaned = tools.clean_html(html);
        assert_eq!(cleaned, "Bold text and italic text");
    }

    #[tokio::test]
    async fn test_invalid_function_call() {
        let tools = WebTools::new();
        let call = FunctionCall {
            name: "invalid_function".to_string(),
            args: HashMap::new(),
        };
        
        let result = tools.execute_function_call(&call).await;
        assert!(result.is_err());
    }
}