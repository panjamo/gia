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
                ContentPart::Text(format!("### Prompt\n\n{}", text))
            }
            ContentPartWrapper::RoleDefinition {
                name,
                content,
                is_task,
            } => {
                let header = if *is_task {
                    format!("### Task: {}", name)
                } else {
                    format!("### Role: {}", name)
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
                    format!("### Content from: {}\n\n{}", path, content)
                } else {
                    format!("### Content from: {}\n\n{}\n", path, content)
                };
                ContentPart::Text(formatted)
            }
            ContentPartWrapper::ClipboardText(text) => {
                ContentPart::Text(format!("### Content from: clipboard\n\n{}", text))
            }
            ContentPartWrapper::StdinText(text) => {
                ContentPart::Text(format!("### Content from: stdin\n\n{}", text))
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
            MessageContentWrapper::Text { text } => {
                genai::chat::MessageContent::from_text(text.clone())
            }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_message_content_wrapper_preserves_multiple_parts() {
        // Create a wrapper with multiple text parts (simulating: prompt + file + clipboard)
        let parts = vec![
            ContentPartWrapper::Prompt("What is this?".to_string()),
            ContentPartWrapper::TextFile {
                path: "data.txt".to_string(),
                content: "Large file content here...".to_string(),
            },
            ContentPartWrapper::ClipboardText("Extra context from clipboard".to_string()),
        ];

        let wrapper = MessageContentWrapper::Parts {
            parts: parts.clone(),
        };

        // Convert to genai MessageContent
        let genai_content = wrapper.to_genai_message_content();

        // Verify that parts are preserved (not merged into single text)
        let result_parts = genai_content.parts();

        // Should have 3 separate parts for Gemini caching
        assert_eq!(
            result_parts.len(),
            3,
            "Parts should be preserved for Gemini caching"
        );

        // All parts should be Text type (with their formatting)
        for part in result_parts {
            assert!(
                matches!(part, genai::chat::ContentPart::Text(_)),
                "All parts should be ContentPart::Text"
            );
        }
    }

    #[test]
    fn test_message_content_wrapper_single_text() {
        let wrapper = MessageContentWrapper::Text {
            text: "Simple text".to_string(),
        };

        let genai_content = wrapper.to_genai_message_content();
        let result_parts = genai_content.parts();

        assert_eq!(result_parts.len(), 1);
        if let genai::chat::ContentPart::Text(text) = &result_parts[0] {
            assert_eq!(text, "Simple text");
        } else {
            panic!("Expected Text part");
        }
    }

    #[test]
    fn test_chat_message_wrapper_preserves_parts_structure() {
        // Simulate a real user message with multiple inputs
        let parts = vec![
            ContentPartWrapper::RoleDefinition {
                name: "code-reviewer".to_string(),
                content: "You are an expert code reviewer.".to_string(),
                is_task: false,
            },
            ContentPartWrapper::TextFile {
                path: "main.rs".to_string(),
                content: "fn main() { println!(\"Hello\"); }".to_string(),
            },
            ContentPartWrapper::Prompt("Review this code".to_string()),
        ];

        let message_wrapper = ChatMessageWrapper {
            role: "User".to_string(),
            content: MessageContentWrapper::Parts { parts },
        };

        // Convert to genai ChatMessage
        let genai_message = message_wrapper.to_genai_chat_message().unwrap();

        // Verify parts are preserved in the final message
        let result_parts = genai_message.content.parts();
        assert_eq!(
            result_parts.len(),
            3,
            "Should preserve 3 separate parts for Gemini caching"
        );

        // Verify the content formatting is correct
        if let genai::chat::ContentPart::Text(text) = &result_parts[0] {
            assert!(
                text.contains("=== Role: code-reviewer ==="),
                "Role definition should be formatted"
            );
        }

        if let genai::chat::ContentPart::Text(text) = &result_parts[1] {
            assert!(
                text.contains("=== Content from: main.rs ==="),
                "File content should be formatted"
            );
        }

        if let genai::chat::ContentPart::Text(text) = &result_parts[2] {
            assert!(
                text.contains("=== Prompt ==="),
                "Prompt should be formatted"
            );
        }
    }

    #[test]
    fn test_message_with_image_preserves_parts() {
        // Test that images are kept as separate parts
        let parts = vec![
            ContentPartWrapper::Image {
                path: Some("photo.jpg".to_string()),
                mime_type: "image/jpeg".to_string(),
                data: "base64data".to_string(),
            },
            ContentPartWrapper::Prompt("What's in this image?".to_string()),
        ];

        let wrapper = MessageContentWrapper::Parts { parts };
        let genai_content = wrapper.to_genai_message_content();
        let result_parts = genai_content.parts();

        assert_eq!(result_parts.len(), 2, "Image and text should be separate");

        // First part should be Binary (image)
        assert!(
            matches!(result_parts[0], genai::chat::ContentPart::Binary(_)),
            "First part should be image"
        );

        // Second part should be Text (prompt)
        assert!(
            matches!(result_parts[1], genai::chat::ContentPart::Text(_)),
            "Second part should be text"
        );
    }
}
