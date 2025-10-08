use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use genai::chat::ChatMessage;
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::constants::CONVERSATION_TRUNCATION_KEEP_MESSAGES;
use crate::content_part_wrapper::ChatMessageWrapper;
use crate::logging::{log_debug, log_info, log_warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ResourceType {
    Image,
    Audio,
    TextFile,
    ClipboardText,
    ClipboardImage,
    Stdin,
    Role,
    Task,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default)]
pub struct TokenUsage {
    pub prompt_tokens: Option<u32>,
    pub completion_tokens: Option<u32>,
    pub total_tokens: Option<u32>,
}

impl TokenUsage {
    pub fn format_short(&self) -> String {
        match (
            self.prompt_tokens,
            self.completion_tokens,
            self.total_tokens,
        ) {
            (Some(p), Some(c), Some(t)) => format!("{}+{}={}", p, c, t),
            (Some(p), Some(c), None) => format!("{}+{}", p, c),
            (None, None, Some(t)) => format!("{}", t),
            _ => "N/A".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceInfo {
    pub resource_type: ResourceType,
    pub path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversationMetadata {
    pub resources_per_message: Vec<Vec<ResourceInfo>>,
    pub model_used: String,
    #[serde(default)]
    pub token_usage_per_message: Vec<TokenUsage>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<ChatMessageWrapper>,
    pub metadata: ConversationMetadata,
}

impl Conversation {
    /// Generate a slug from the first prompt text
    /// Takes first 3-5 significant words, max 40 chars, kebab-case
    fn generate_slug(prompt: &str) -> String {
        const STOPWORDS: &[&str] = &[
            "the", "a", "an", "is", "are", "was", "were", "be", "been", "being", "have", "has",
            "had", "do", "does", "did", "will", "would", "should", "could", "can", "may", "might",
            "must", "shall", "how", "what", "when", "where", "who", "why", "which", "this", "that",
            "these", "those", "i", "you", "he", "she", "it", "we", "they", "me", "him", "her",
            "us", "them", "my", "your", "his", "its", "our", "their",
        ];

        let lowercase = prompt.to_lowercase();
        let words: Vec<&str> = lowercase
            .split_whitespace()
            .filter(|word| {
                let cleaned = word.trim_matches(|c: char| !c.is_alphanumeric());
                !cleaned.is_empty() && !STOPWORDS.contains(&cleaned)
            })
            .take(5)
            .collect();

        if words.is_empty() {
            return "conversation".to_string();
        }

        let slug = words
            .iter()
            .map(|word| {
                word.chars()
                    .filter(|c| c.is_alphanumeric() || *c == '-')
                    .collect::<String>()
            })
            .collect::<Vec<_>>()
            .join("-");

        // Truncate to 40 chars max
        if slug.len() > 40 {
            slug.chars().take(40).collect()
        } else {
            slug
        }
    }

    /// Create a new conversation with a slug-hash ID based on the first prompt
    pub fn new_with_prompt(model_name: String, first_prompt: &str) -> Self {
        let now = Utc::now();
        let uuid = Uuid::new_v4();
        let slug = Self::generate_slug(first_prompt);
        let hash4 = &uuid.to_string()[..4];
        let id = format!("{}-{}", slug, hash4);

        Self {
            id,
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
            metadata: ConversationMetadata {
                resources_per_message: Vec::new(),
                model_used: model_name,
                token_usage_per_message: Vec::new(),
            },
        }
    }

    #[cfg(test)]
    pub fn new(model_name: String) -> Self {
        // Fallback for tests and cases where we don't have a prompt yet
        Self::new_with_prompt(model_name, "conversation")
    }

    pub fn add_message_with_usage(
        &mut self,
        message: ChatMessageWrapper,
        resources: Vec<ResourceInfo>,
        usage: TokenUsage,
    ) {
        self.messages.push(message);
        self.metadata.resources_per_message.push(resources);
        self.metadata.token_usage_per_message.push(usage);
        self.updated_at = Utc::now();
    }

    /// Convert wrapper messages to genai ChatMessages for API calls
    pub fn to_genai_messages(&self) -> Result<Vec<ChatMessage>> {
        self.messages
            .iter()
            .map(|wrapper| wrapper.to_genai_chat_message())
            .collect()
    }

    pub fn truncate_if_needed(&mut self, max_length: usize) {
        let current_length = self.estimate_token_length();
        if current_length <= max_length {
            return;
        }

        log_info(&format!(
            "Conversation too long ({current_length} chars), truncating to fit context window"
        ));

        // Keep removing oldest messages until we're under the limit
        while self.estimate_token_length() > max_length
            && self.messages.len() > CONVERSATION_TRUNCATION_KEEP_MESSAGES
        {
            self.messages.remove(0);
            self.metadata.resources_per_message.remove(0);
            self.metadata.token_usage_per_message.remove(0);
            log_debug("Removed oldest message to fit context window");
        }

        // If still too long and we have messages, keep only the most recent pair
        if self.estimate_token_length() > max_length
            && self.messages.len() > CONVERSATION_TRUNCATION_KEEP_MESSAGES
        {
            let last_messages = self
                .messages
                .split_off(self.messages.len() - CONVERSATION_TRUNCATION_KEEP_MESSAGES);
            let last_resources = self
                .metadata
                .resources_per_message
                .split_off(self.messages.len());
            let last_token_usage = self
                .metadata
                .token_usage_per_message
                .split_off(self.messages.len());

            self.messages = last_messages;
            self.metadata.resources_per_message = last_resources;
            self.metadata.token_usage_per_message = last_token_usage;

            log_warn(&format!(
                "Had to truncate conversation to only the last {CONVERSATION_TRUNCATION_KEEP_MESSAGES} messages"
            ));
        }
    }

    fn estimate_token_length(&self) -> usize {
        // Rough estimation: ~4 characters per token
        let content_length: usize = self
            .messages
            .iter()
            .map(|m| Self::estimate_message_length(m) + 20) // +20 for role prefix and formatting
            .sum();
        content_length
    }

    fn estimate_message_length(message: &ChatMessageWrapper) -> usize {
        use crate::content_part_wrapper::MessageContentWrapper;

        match &message.content {
            MessageContentWrapper::Text { text } => text.len(),
            MessageContentWrapper::Parts { parts } => parts
                .iter()
                .map(|part| part.extract_text().map(|t| t.len()).unwrap_or(100))
                .sum(),
        }
    }

    /// Extract text content from a ChatMessageWrapper
    pub fn extract_text_content(message: &ChatMessageWrapper) -> String {
        use crate::content_part_wrapper::MessageContentWrapper;

        match &message.content {
            MessageContentWrapper::Text { text } => text.clone(),
            MessageContentWrapper::Parts { parts } => parts
                .iter()
                .filter_map(|part| part.extract_text())
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    /// Extract only the Prompt section from a message (for TTS)
    pub fn extract_prompt_section(message: &ChatMessageWrapper) -> String {
        use crate::content_part_wrapper::MessageContentWrapper;

        match &message.content {
            MessageContentWrapper::Text { text } => text.clone(),
            MessageContentWrapper::Parts { parts } => parts
                .iter()
                .filter_map(|part| part.extract_prompt())
                .collect::<Vec<_>>()
                .join("\n"),
        }
    }

    pub fn format_as_chat_markdown(&self) -> String {
        let username = whoami::username();
        let mut markdown = String::new();

        // Add conversation header
        write!(markdown, "### Conversation {}\n\n", self.id).unwrap();
        writeln!(
            markdown,
            "**Created:** {}",
            self.created_at.format("%Y-%m-%d %H:%M:%S UTC")
        )
        .unwrap();
        writeln!(
            markdown,
            "**Updated:** {}",
            self.updated_at.format("%Y-%m-%d %H:%M:%S UTC")
        )
        .unwrap();
        write!(markdown, "**Messages:** {}\n\n", self.messages.len()).unwrap();
        writeln!(markdown, "**Model:** {}\n", self.metadata.model_used).unwrap();
        markdown.push_str("---\n\n");

        // Add messages with metadata
        for (i, ((message, resources), usage)) in self
            .messages
            .iter()
            .zip(self.metadata.resources_per_message.iter())
            .zip(self.metadata.token_usage_per_message.iter())
            .enumerate()
        {
            if i > 0 {
                markdown.push_str("\n---\n\n");
            }

            match message.role.as_str() {
                "User" => {
                    // Extract text content from message
                    let text_content = Self::extract_text_content(message);
                    let escaped_content =
                        html_escape::encode_text(&text_content).replace('\n', "<br>");

                    // Build resources list if any
                    let mut resources_html = String::new();
                    if !resources.is_empty() {
                        resources_html
                            .push_str("<p><small><strong>Resources:</strong></small></p><ul>");
                        for resource in resources {
                            let resource_text = match &resource.resource_type {
                                ResourceType::Image => {
                                    if let Some(path) = &resource.path {
                                        format!("📷 Image: {}", path)
                                    } else {
                                        "📷 Image".to_string()
                                    }
                                }
                                ResourceType::Audio => {
                                    if let Some(path) = &resource.path {
                                        format!("🎤 Audio: {}", path)
                                    } else {
                                        "🎤 Audio".to_string()
                                    }
                                }
                                ResourceType::TextFile => {
                                    if let Some(path) = &resource.path {
                                        format!("📄 File: {}", path)
                                    } else {
                                        "📄 File".to_string()
                                    }
                                }
                                ResourceType::ClipboardText => "📋 Clipboard text".to_string(),
                                ResourceType::ClipboardImage => "📋 Clipboard image".to_string(),
                                ResourceType::Stdin => "⌨️  Stdin input".to_string(),
                                ResourceType::Role => {
                                    if let Some(role_name) = &resource.path {
                                        format!("🎭 Role: {}", role_name)
                                    } else {
                                        "🎭 Role".to_string()
                                    }
                                }
                                ResourceType::Task => {
                                    if let Some(task_name) = &resource.path {
                                        format!("✅ Task: {}", task_name)
                                    } else {
                                        "✅ Task".to_string()
                                    }
                                }
                            };
                            let escaped_resource = html_escape::encode_text(&resource_text);
                            resources_html.push_str(&format!("<li>{}</li>", escaped_resource));
                        }
                        resources_html.push_str("</ul>");
                    }

                    markdown.push_str(&format!(
                        r#"<div class="gia-prompt">
<h3>💬 {}</h3>
<p>{}</p>
{}
</div>

"#,
                        username, escaped_content, resources_html
                    ));
                }
                "Assistant" => {
                    let text_content = Self::extract_text_content(message);
                    markdown.push_str("**Assistant:** ");
                    markdown.push_str(&text_content);

                    // Add token usage information for assistant responses
                    if usage.prompt_tokens.is_some()
                        || usage.completion_tokens.is_some()
                        || usage.total_tokens.is_some()
                    {
                        markdown.push_str(&format!(
                            "\n\n<small>📊 **Tokens:** {}</small>",
                            usage.format_short()
                        ));
                    }

                    markdown.push('\n');
                }
                _ => {
                    // Ignore System/Tool messages in markdown output
                }
            }

            markdown.push_str(&format!(
                "\n*{}*\n",
                Utc::now().format("%Y-%m-%d %H:%M:%S UTC")
            ));
        }

        markdown
    }
}

pub struct ConversationManager {
    conversations_dir: PathBuf,
}

impl ConversationManager {
    pub fn new() -> Result<Self> {
        let conversations_dir = Self::get_conversations_dir()?;

        // Ensure the conversations directory exists
        if !conversations_dir.exists() {
            fs::create_dir_all(&conversations_dir)
                .context("Failed to create conversations directory")?;
            log_info(&format!(
                "Created conversations directory: {conversations_dir:?}"
            ));
        }

        Ok(Self { conversations_dir })
    }

    fn get_conversations_dir() -> Result<PathBuf> {
        let home_dir =
            dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not find home directory"))?;
        Ok(home_dir.join(".gia").join("conversations"))
    }

    pub fn save_conversation(&self, conversation: &Conversation) -> Result<()> {
        let filename = format!("{}.json", conversation.id);
        let file_path = self.conversations_dir.join(filename);

        let json_content = serde_json::to_string_pretty(conversation)
            .context("Failed to serialize conversation")?;

        fs::write(&file_path, json_content).context("Failed to write conversation file")?;

        log_debug(&format!("Saved conversation to: {file_path:?}"));
        Ok(())
    }

    pub fn save_markdown(&self, conversation: &Conversation) -> Result<()> {
        let filename = format!("{}.md", conversation.id);
        let file_path = self.conversations_dir.join(filename);
        let markdown = conversation.format_as_chat_markdown();
        fs::write(&file_path, markdown).context("Failed to write markdown file")?;
        log_debug(&format!("Saved markdown to: {file_path:?}"));
        Ok(())
    }

    pub fn get_markdown_path(&self, conversation: &Conversation) -> Result<PathBuf> {
        let filename = format!("{}.md", conversation.id);
        Ok(self.conversations_dir.join(filename))
    }

    pub fn load_conversation(&self, id: &str) -> Result<Conversation> {
        // Check if id is a number (relative index)
        if let Ok(index) = id.parse::<usize>() {
            return self.load_conversation_by_index(index);
        }

        // Try exact match first
        let filename = format!("{id}.json");
        let file_path = self.conversations_dir.join(&filename);

        if file_path.exists() {
            let content =
                fs::read_to_string(&file_path).context("Failed to read conversation file")?;
            let conversation: Conversation =
                serde_json::from_str(&content).context("Failed to deserialize conversation")?;
            log_debug(&format!("Loaded conversation from: {file_path:?}"));
            return Ok(conversation);
        }

        // If not found, try matching by hash suffix (last 4 chars before .json)
        let entries = fs::read_dir(&self.conversations_dir)
            .context("Failed to read conversations directory")?;

        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                    // Check if the name ends with the provided id (hash match)
                    if name.ends_with(id) {
                        let content = fs::read_to_string(&path)
                            .context("Failed to read conversation file")?;
                        let conversation: Conversation = serde_json::from_str(&content)
                            .context("Failed to deserialize conversation")?;
                        log_debug(&format!("Loaded conversation from: {path:?}"));
                        return Ok(conversation);
                    }
                }
            }
        }

        Err(anyhow::anyhow!("Conversation with ID '{id}' not found"))
    }

    fn load_conversation_by_index(&self, index: usize) -> Result<Conversation> {
        let summaries = self.list_conversations()?;

        if index >= summaries.len() {
            return Err(anyhow::anyhow!(
                "Conversation index {} out of range (have {} conversations)",
                index,
                summaries.len()
            ));
        }

        let conversation_id = &summaries[index].id;
        let filename = format!("{conversation_id}.json");
        let file_path = self.conversations_dir.join(filename);

        let content = fs::read_to_string(&file_path).context("Failed to read conversation file")?;
        let conversation: Conversation =
            serde_json::from_str(&content).context("Failed to deserialize conversation")?;
        log_debug(&format!(
            "Loaded conversation [{}] from: {file_path:?}",
            index
        ));
        Ok(conversation)
    }

    pub fn get_latest_conversation(&self) -> Result<Option<Conversation>> {
        let mut latest_conversation: Option<Conversation> = None;
        let mut latest_time = DateTime::<Utc>::MIN_UTC;

        // Read all conversation files
        let entries = fs::read_dir(&self.conversations_dir)
            .context("Failed to read conversations directory")?;

        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match Self::load_conversation_from_path(&path) {
                    Ok(conversation) => {
                        if conversation.updated_at > latest_time {
                            latest_time = conversation.updated_at;
                            latest_conversation = Some(conversation);
                        }
                    }
                    Err(e) => {
                        log_warn(&format!("Failed to load conversation from {path:?}: {e}"));
                    }
                }
            }
        }

        Ok(latest_conversation)
    }

    pub fn list_conversations(&self) -> Result<Vec<ConversationSummary>> {
        let mut summaries = Vec::new();

        let entries = fs::read_dir(&self.conversations_dir)
            .context("Failed to read conversations directory")?;

        for entry in entries {
            let entry = entry.context("Failed to read directory entry")?;
            let path = entry.path();

            if path.extension().and_then(|s| s.to_str()) == Some("json") {
                match Self::load_conversation_from_path(&path) {
                    Ok(conversation) => {
                        let summary = ConversationSummary::from_conversation(&conversation);
                        summaries.push(summary);
                    }
                    Err(e) => {
                        log_warn(&format!("Failed to load conversation from {path:?}: {e}"));
                    }
                }
            }
        }

        // Sort by updated_at descending (newest first)
        summaries.sort_by(|a, b| b.updated_at.cmp(&a.updated_at));
        Ok(summaries)
    }

    fn load_conversation_from_path(path: &Path) -> Result<Conversation> {
        let content = fs::read_to_string(path).context("Failed to read conversation file")?;

        let conversation: Conversation =
            serde_json::from_str(&content).context("Failed to deserialize conversation")?;

        Ok(conversation)
    }
}

#[derive(Debug)]
pub struct ConversationSummary {
    pub id: String,
    #[allow(dead_code)]
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[allow(dead_code)]
    pub message_count: usize,
    pub first_user_message: Option<String>,
}

impl ConversationSummary {
    pub fn from_conversation(conversation: &Conversation) -> Self {
        let first_user_message = conversation
            .messages
            .iter()
            .find(|m| m.role == "User")
            .map(|m| {
                let content = Conversation::extract_text_content(m);
                // Truncate to first 50 characters for summary
                if content.len() > 50 {
                    format!("{}...", &content[..47])
                } else {
                    content
                }
            });

        Self {
            id: conversation.id.clone(),
            created_at: conversation.created_at,
            updated_at: conversation.updated_at,
            message_count: conversation.messages.len(),
            first_user_message,
        }
    }

    /// Format conversation data as separate columns for tabwriter
    /// Returns (preview, id, age, messages) tuple
    pub fn format_as_table_columns(&self) -> (String, String, String, String) {
        let age = Utc::now() - self.updated_at;
        let age_str = if age.num_days() > 0 {
            format!("{}d", age.num_days())
        } else if age.num_hours() > 0 {
            format!("{}h", age.num_hours())
        } else {
            format!("{}m", age.num_minutes().max(1))
        };

        let default_message = "(no messages)".to_string();
        let preview = self.first_user_message.as_ref().unwrap_or(&default_message);

        // Replace line feeds and tabs with spaces for table format
        let preview_clean = preview.replace(['\n', '\r', '\t'], " ");

        (
            preview_clean,
            self.id.clone(),
            age_str,
            self.message_count.to_string(),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_creation() {
        let conversation = Conversation::new("test-model".to_string());
        assert!(!conversation.id.is_empty());
        assert_eq!(conversation.messages.len(), 0);
        assert_eq!(conversation.metadata.model_used, "test-model");
    }

    #[test]
    fn test_add_message() {
        use crate::content_part_wrapper::{ChatMessageWrapper, MessageContentWrapper};

        let mut conversation = Conversation::new("test-model".to_string());
        let message = ChatMessageWrapper {
            role: "User".to_string(),
            content: MessageContentWrapper::Text {
                text: "Hello".to_string(),
            },
        };
        conversation.add_message_with_usage(message, Vec::new(), TokenUsage::default());

        assert_eq!(conversation.messages.len(), 1);
        assert_eq!(conversation.messages[0].role, "User");
        assert_eq!(conversation.metadata.resources_per_message.len(), 1);
        assert_eq!(conversation.metadata.token_usage_per_message.len(), 1);
    }

    #[test]
    fn test_extract_text_content() {
        use crate::content_part_wrapper::{ChatMessageWrapper, MessageContentWrapper};

        let message = ChatMessageWrapper {
            role: "User".to_string(),
            content: MessageContentWrapper::Text {
                text: "Hello world".to_string(),
            },
        };
        let text = Conversation::extract_text_content(&message);
        assert_eq!(text, "Hello world");
    }

    #[test]
    fn test_extract_prompt_section() {
        use crate::content_part_wrapper::{
            ChatMessageWrapper, ContentPartWrapper, MessageContentWrapper,
        };

        let message = ChatMessageWrapper {
            role: "User".to_string(),
            content: MessageContentWrapper::Parts {
                parts: vec![ContentPartWrapper::Prompt("My prompt".to_string())],
            },
        };
        let prompt = Conversation::extract_prompt_section(&message);
        assert_eq!(prompt, "My prompt");
    }

    #[test]
    fn test_truncate_if_needed() {
        use crate::content_part_wrapper::{ChatMessageWrapper, MessageContentWrapper};

        let mut conversation = Conversation::new("test-model".to_string());

        // Add more than CONVERSATION_TRUNCATION_KEEP_MESSAGES (20) to test truncation
        for i in 0..25 {
            let role = if i % 2 == 0 { "User" } else { "Assistant" };
            let message = ChatMessageWrapper {
                role: role.to_string(),
                content: MessageContentWrapper::Text {
                    text: format!("Message {}", i).repeat(100),
                },
            };
            conversation.add_message_with_usage(message, Vec::new(), TokenUsage::default());
        }

        let initial_count = conversation.messages.len();
        assert_eq!(initial_count, 25);

        // Truncate to fit in ~2000 chars (should keep only last 20 messages due to minimum)
        conversation.truncate_if_needed(2000);

        // Should have fewer messages now, but at least CONVERSATION_TRUNCATION_KEEP_MESSAGES
        assert!(conversation.messages.len() < initial_count);
        assert!(conversation.messages.len() >= 20); // At least the minimum
    }

    #[test]
    fn test_generate_slug() {
        // Test basic slug generation
        let slug = Conversation::generate_slug("How does the multi-API key fallback work?");
        assert!(slug.contains("multi"));
        assert!(slug.contains("api"));
        assert!(slug.contains("key"));

        // Test stopword filtering (take max 5 words after filtering)
        let slug = Conversation::generate_slug("I want to fix the bug in the clipboard");
        assert!(slug.contains("want")); // "want" is a stopword, should be filtered
        assert!(slug.contains("fix"));
        assert!(slug.contains("bug"));
        // Only takes first 5 significant words, so "clipboard" might not be included

        // Test max 5 words
        let slug = Conversation::generate_slug("one two three four five six seven eight");
        let word_count = slug.split('-').count();
        assert!(word_count <= 5);

        // Test max 40 chars
        let slug = Conversation::generate_slug("superlongword ".repeat(10).as_str());
        assert!(slug.len() <= 40);

        // Test empty/only stopwords
        let slug = Conversation::generate_slug("the a an is");
        assert_eq!(slug, "conversation");

        // Test special characters removal
        let slug = Conversation::generate_slug("Fix bug clipboard image handling now");
        assert!(slug.contains("fix"));
        assert!(slug.contains("bug"));
        // Test that special chars are removed
        assert!(!slug.contains('@'));
        assert!(!slug.contains('#'));
        assert!(!slug.contains('!'));
    }

    #[test]
    fn test_new_with_prompt() {
        let conversation =
            Conversation::new_with_prompt("test-model".to_string(), "Debug the clipboard handling");

        // Should contain slug and hash
        assert!(conversation.id.contains("debug"));
        assert!(conversation.id.contains("clipboard"));
        assert!(conversation.id.contains("handling"));

        // Should have a dash followed by 4-char hash at the end
        let parts: Vec<&str> = conversation.id.rsplitn(2, '-').collect();
        assert_eq!(parts.len(), 2);
        assert_eq!(parts[0].len(), 4); // hash part
    }

    #[test]
    fn test_conversation_id_format() {
        let conversation =
            Conversation::new_with_prompt("test-model".to_string(), "Fix API rate limiting");

        // ID format: {slug}-{hash4}
        // Should match pattern: word-word-word-xxxx
        let id_parts: Vec<&str> = conversation.id.split('-').collect();
        assert!(id_parts.len() >= 2); // At least one word + hash

        // Last part should be 4 chars (the hash)
        let hash_part = id_parts.last().unwrap();
        assert_eq!(hash_part.len(), 4);

        // All characters should be lowercase alphanumeric or dash
        for c in conversation.id.chars() {
            assert!(c.is_lowercase() || c.is_numeric() || c == '-');
        }
    }
}
