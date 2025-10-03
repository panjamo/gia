use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::fmt::Write;
use std::fs;
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::constants::CONVERSATION_TRUNCATION_KEEP_MESSAGES;
use crate::logging::{log_debug, log_info, log_warn};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: MessageRole,
    pub content: String,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conversation {
    pub id: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub messages: Vec<Message>,
}

impl Conversation {
    pub fn new() -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            created_at: now,
            updated_at: now,
            messages: Vec::new(),
        }
    }

    pub fn add_message(&mut self, role: MessageRole, content: String) {
        let message = Message {
            role,
            content,
            timestamp: Utc::now(),
        };
        self.messages.push(message);
        self.updated_at = Utc::now();
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
            log_debug("Removed oldest message to fit context window");
        }

        // If still too long and we have messages, keep only the most recent pair
        if self.estimate_token_length() > max_length
            && self.messages.len() > CONVERSATION_TRUNCATION_KEEP_MESSAGES
        {
            let last_messages = self
                .messages
                .split_off(self.messages.len() - CONVERSATION_TRUNCATION_KEEP_MESSAGES);
            self.messages = last_messages;
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
            .map(|m| m.content.len() + 20) // +20 for role prefix and formatting
            .sum();
        content_length
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
        markdown.push_str("---\n\n");

        // Add messages
        for (i, message) in self.messages.iter().enumerate() {
            if i > 0 {
                markdown.push_str("\n---\n\n");
            }

            match message.role {
                MessageRole::User => {
                    // Format user prompts in a styled box similar to browser output
                    // Use raw HTML with escaped content
                    let escaped_content = html_escape::encode_text(&message.content).replace('\n', "<br>");
                    markdown.push_str(&format!(
                        r#"<div class="gia-prompt">
<h3>💬 {}</h3>
<p>{}</p>
</div>

"#,
                        username,
                        escaped_content
                    ));
                }
                MessageRole::Assistant => {
                    markdown.push_str("**Assistant:** ");
                    markdown.push_str(&message.content);
                    markdown.push_str("\n");
                }
            }

            write!(
                markdown,
                "\n*{}*\n",
                message.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
            )
            .unwrap();
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
        use crate::output::{get_outputs_dir, sanitize_prompt_for_filename};

        let prompt = conversation
            .messages
            .iter()
            .find(|m| matches!(m.role, MessageRole::User))
            .map(|m| m.content.as_str())
            .unwrap_or("");

        let outputs_dir = get_outputs_dir()?;
        if !outputs_dir.exists() {
            fs::create_dir_all(&outputs_dir).context("Failed to create outputs directory")?;
        }

        let sanitized = sanitize_prompt_for_filename(prompt);
        let sanitized = if sanitized.is_empty() {
            "conversation".to_string()
        } else {
            sanitized
        };

        let conv_prefix = &conversation.id[..8];
        let filename = format!("{}_{}.md", conv_prefix, sanitized);
        let file_path = outputs_dir.join(filename);
        let markdown = conversation.format_as_chat_markdown();
        fs::write(&file_path, markdown).context("Failed to write markdown file")?;
        log_debug(&format!("Saved markdown to: {file_path:?}"));
        Ok(())
    }

    pub fn load_conversation(&self, id: &str) -> Result<Conversation> {
        let filename = format!("{id}.json");
        let file_path = self.conversations_dir.join(filename);

        if !file_path.exists() {
            return Err(anyhow::anyhow!("Conversation with ID '{id}' not found"));
        }

        let content = fs::read_to_string(&file_path).context("Failed to read conversation file")?;
        let conversation: Conversation =
            serde_json::from_str(&content).context("Failed to deserialize conversation")?;
        log_debug(&format!("Loaded conversation from: {file_path:?}"));
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
    pub message_count: usize,
    pub first_user_message: Option<String>,
}

impl ConversationSummary {
    pub fn from_conversation(conversation: &Conversation) -> Self {
        let first_user_message = conversation
            .messages
            .iter()
            .find(|m| matches!(m.role, MessageRole::User))
            .map(|m| {
                // Truncate to first 50 characters for summary
                if m.content.len() > 50 {
                    format!("{}...", &m.content[..47])
                } else {
                    m.content.clone()
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

    pub fn format_as_table_row(&self) -> String {
        let age = Utc::now() - self.updated_at;
        let age_str = if age.num_days() > 0 {
            format!("{}d ago", age.num_days())
        } else if age.num_hours() > 0 {
            format!("{}h ago", age.num_hours())
        } else {
            format!("{}m ago", age.num_minutes().max(1))
        };

        let default_message = "(no messages)".to_string();
        let preview = self.first_user_message.as_ref().unwrap_or(&default_message);

        // Replace line feeds and tabs with spaces for table format
        let preview_clean = preview.replace(['\n', '\r', '\t'], " ");

        let updated_str = self.updated_at.format("%Y-%m-%d %H:%M:%S").to_string();

        format!(
            "{}\t{}\t{}\t{}\t{}",
            self.id, self.message_count, updated_str, age_str, preview_clean
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_conversation_creation() {
        let conversation = Conversation::new();
        assert!(!conversation.id.is_empty());
        assert_eq!(conversation.messages.len(), 0);
    }

    #[test]
    fn test_add_message() {
        let mut conversation = Conversation::new();
        conversation.add_message(MessageRole::User, "Hello".to_string());

        assert_eq!(conversation.messages.len(), 1);
        assert!(matches!(conversation.messages[0].role, MessageRole::User));
        assert_eq!(conversation.messages[0].content, "Hello");
    }

    #[test]
    fn test_truncate_if_needed() {
        let mut conversation = Conversation::new();
        conversation.add_message(MessageRole::User, "A".repeat(1000));
        conversation.add_message(MessageRole::Assistant, "B".repeat(1000));
        conversation.add_message(MessageRole::User, "C".repeat(1000));
        conversation.add_message(MessageRole::Assistant, "D".repeat(1000));

        conversation.truncate_if_needed(2500); // Should keep last 2 messages

        assert_eq!(conversation.messages.len(), 2);
        assert!(conversation.messages[0].content.contains('C'));
        assert!(conversation.messages[1].content.contains('D'));
    }
}
