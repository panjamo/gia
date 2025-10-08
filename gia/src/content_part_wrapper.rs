/// Custom ContentPart wrapper for structured serialization
/// This allows us to save conversations with strongly-typed content parts
/// instead of parsing text with === markers
use anyhow::Result;
use genai::chat::ContentPart;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
pub enum ContentPartWrapper {
    /// Main user prompt
    Prompt(String),

    /// Role or task definition
    RoleDefinition {
        name: String,
        content: String,
        is_task: bool,
    },

    /// Text file content
    TextFile { path: String, content: String },

    /// Clipboard text
    ClipboardText(String),

    /// Stdin text
    StdinText(String),

    /// Image as base64
    Image {
        path: Option<String>, // Original file path if from file
        mime_type: String,
        data: String, // base64
    },

    /// Audio as base64
    Audio {
        path: String,
        mime_type: String,
        data: String, // base64
    },

    /// Plain text (for any other text content)
    Text(String),
}

impl ContentPartWrapper {
    /// Convert to genai::ContentPart for API requests
    pub fn to_genai_content_part(&self) -> ContentPart {
        match self {
            ContentPartWrapper::Prompt(text) => {
                ContentPart::Text(format!("=== Prompt ===\n\n{}", text))
            }
            ContentPartWrapper::RoleDefinition {
                name,
                content,
                is_task,
            } => {
                let header = if *is_task {
                    format!("=== Task: {} ===", name)
                } else {
                    format!("=== Role: {} ===", name)
                };
                let formatted = if content.ends_with('\n') {
                    format!("{}\n{}", header, content)
                } else {
                    format!("{}\n{}\n", header, content)
                };
                ContentPart::Text(formatted)
            }
            ContentPartWrapper::TextFile { path, content } => {
                let formatted = if content.ends_with('\n') {
                    format!("=== Content from: {} ===\n{}", path, content)
                } else {
                    format!("=== Content from: {} ===\n{}\n", path, content)
                };
                ContentPart::Text(formatted)
            }
            ContentPartWrapper::ClipboardText(text) => {
                ContentPart::Text(format!("=== Content from: clipboard ===\n{}", text))
            }
            ContentPartWrapper::StdinText(text) => {
                ContentPart::Text(format!("=== Content from: stdin ===\n{}", text))
            }
            ContentPartWrapper::Image {
                mime_type, data, ..
            } => ContentPart::from_binary_base64(mime_type.clone(), data.clone(), None),
            ContentPartWrapper::Audio {
                mime_type, data, ..
            } => ContentPart::from_binary_base64(mime_type.clone(), data.clone(), None),
            ContentPartWrapper::Text(text) => ContentPart::Text(text.clone()),
        }
    }

    /// Extract text content for display/TTS
    pub fn extract_text(&self) -> Option<String> {
        match self {
            ContentPartWrapper::Prompt(text) => Some(text.clone()),
            ContentPartWrapper::RoleDefinition { content, .. } => Some(content.clone()),
            ContentPartWrapper::TextFile { content, .. } => Some(content.clone()),
            ContentPartWrapper::ClipboardText(text) => Some(text.clone()),
            ContentPartWrapper::StdinText(text) => Some(text.clone()),
            ContentPartWrapper::Text(text) => Some(text.clone()),
            ContentPartWrapper::Image { .. } | ContentPartWrapper::Audio { .. } => None,
        }
    }

    /// Extract only prompt text (for TTS)
    pub fn extract_prompt(&self) -> Option<String> {
        match self {
            ContentPartWrapper::Prompt(text) => Some(text.clone()),
            _ => None,
        }
    }
}

/// Wrapper for message content that uses our custom ContentPartWrapper
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MessageContentWrapper {
    Text { text: String },
    Parts { parts: Vec<ContentPartWrapper> },
}

impl MessageContentWrapper {
    /// Convert to genai::MessageContent for API requests
    pub fn to_genai_message_content(&self) -> genai::chat::MessageContent {
        match self {
            MessageContentWrapper::Text { text } => genai::chat::MessageContent::from_text(text.clone()),
            MessageContentWrapper::Parts { parts } => {
                let content_parts: Vec<ContentPart> =
                    parts.iter().map(|p| p.to_genai_content_part()).collect();
                genai::chat::MessageContent::from_parts(content_parts)
            }
        }
    }
}

/// Custom ChatMessage wrapper for serialization
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessageWrapper {
    pub role: String, // "User", "Assistant", "System"
    pub content: MessageContentWrapper,
}

impl ChatMessageWrapper {
    /// Convert to genai::ChatMessage for API requests
    pub fn to_genai_chat_message(&self) -> Result<genai::chat::ChatMessage> {
        use genai::chat::{ChatMessage, ChatRole};

        let role = match self.role.as_str() {
            "User" => ChatRole::User,
            "Assistant" => ChatRole::Assistant,
            "System" => ChatRole::System,
            "Tool" => ChatRole::Tool,
            _ => ChatRole::User, // Default to User
        };

        let content = self.content.to_genai_message_content();

        Ok(ChatMessage {
            role,
            content,
            options: None,
        })
    }
}
