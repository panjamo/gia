//! Tool registry for LLM function calling.
//!
//! To add a new tool:
//! 1. Create a struct implementing `GiaTool`
//! 2. Add it to `all_tools()`
//!
//! Each tool gets a `definition()` (advertised to the LLM) and an `execute()` that
//! runs when the LLM calls it. Results carry a mandatory text part (returned as the
//! ToolResponse) and an optional binary blob (injected as a follow-up User message,
//! because the genai ToolResponse type is text-only).

use anyhow::Result;
use genai::chat::Tool;
use serde_json::{Value, json};

use crate::logging::{log_error, log_info};

// ── Result type ─────────────────────────────────────────────────────────────

/// Binary content returned by a tool (image, audio, …).
/// `gemini.rs` converts this into a `ContentPart::Binary` injected as a User
/// message so the model can process the raw data.
pub struct BinaryContent {
    pub mime_type: String,
    pub base64: String,
}

/// The result of executing a tool.
pub struct ToolResult {
    /// Text returned to the model as the ToolResponse body.
    pub text: String,
    /// Optional binary blob injected as a subsequent User message.
    pub binary: Option<BinaryContent>,
}

impl ToolResult {
    pub fn text(text: impl Into<String>) -> Self {
        Self { text: text.into(), binary: None }
    }

    pub fn with_binary(text: impl Into<String>, mime_type: impl Into<String>, base64: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            binary: Some(BinaryContent { mime_type: mime_type.into(), base64: base64.into() }),
        }
    }
}

// ── Trait ────────────────────────────────────────────────────────────────────

pub trait GiaTool: Send + Sync {
    /// The unique function name used in the LLM protocol (must match `definition().name`).
    fn name(&self) -> &str;

    /// The tool definition advertised to the LLM.
    fn definition(&self) -> Tool;

    /// Execute the tool with the arguments provided by the LLM.
    fn execute(&self, args: &Value) -> Result<ToolResult>;
}

// ── Registry ─────────────────────────────────────────────────────────────────

/// Returns all tools that are advertised to the LLM.
/// Add new tools here.
pub fn all_tools() -> Vec<Box<dyn GiaTool>> {
    vec![
        Box::new(ClipboardTool),
    ]
}

// ── ClipboardTool ─────────────────────────────────────────────────────────────

pub struct ClipboardTool;

impl GiaTool for ClipboardTool {
    fn name(&self) -> &str {
        "get_clipboard_content"
    }

    fn definition(&self) -> Tool {
        Tool::new("get_clipboard_content")
            .with_description(
                "Reads the current content of the user's clipboard (Zwischenablage). \
                 Returns text if the clipboard contains text, or an image if it contains \
                 an image. Call this tool when the user mentions that you should use the \
                 clipboard, Zwischenablage, or clipboard content.",
            )
            .with_schema(json!({
                "type": "object",
                "properties": {},
                "required": []
            }))
    }

    fn execute(&self, _args: &Value) -> Result<ToolResult> {
        // Prefer image over text when both could be present
        match crate::clipboard::has_clipboard_image() {
            Ok(true) => {
                match crate::clipboard::read_clipboard_image()
                    .and_then(|img| crate::clipboard::convert_image_data_to_base64(&img))
                {
                    Ok(base64) => {
                        eprintln!("📋 Clipboard image requested by AI (sending as PNG)");
                        log_info("Clipboard image retrieved, injecting as binary content");
                        Ok(ToolResult::with_binary(
                            "Clipboard contains an image (PNG). \
                             It is attached as binary data in the following message.",
                            "image/png",
                            base64,
                        ))
                    }
                    Err(e) => {
                        log_error(&format!("Failed to read clipboard image: {e}"));
                        Ok(ToolResult::text(format!("(Error reading clipboard image: {e})")))
                    }
                }
            }
            _ => {
                // No image (or error detecting one) – fall back to text
                match crate::clipboard::read_clipboard() {
                    Ok(text) if !text.is_empty() => {
                        eprintln!("📋 Clipboard content requested by AI ({} chars)", text.len());
                        log_info(&format!("Clipboard text retrieved: {} chars", text.len()));
                        Ok(ToolResult::text(text))
                    }
                    Ok(_) => {
                        eprintln!("📋 Clipboard is empty");
                        Ok(ToolResult::text("(Clipboard is empty)"))
                    }
                    Err(e) => {
                        log_error(&format!("Failed to read clipboard: {e}"));
                        Ok(ToolResult::text(format!("(Error reading clipboard: {e})")))
                    }
                }
            }
        }
    }
}
