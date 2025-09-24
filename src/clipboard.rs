use crate::logging::{log_debug, log_info};
use anyhow::{Context, Result};
use arboard::Clipboard;

pub struct ClipboardManager {
    clipboard: Clipboard,
}

impl ClipboardManager {
    pub fn new() -> Result<Self> {
        log_debug("Initializing clipboard manager");
        let clipboard = Clipboard::new().context("Failed to initialize clipboard")?;

        Ok(Self { clipboard })
    }

    pub fn get_text(&mut self) -> Result<String> {
        log_debug("Reading text from clipboard");

        let text = self
            .clipboard
            .get_text()
            .context("Failed to read text from clipboard")?;

        log_info(&format!("Read {} characters from clipboard", text.len()));
        Ok(text)
    }

    pub fn set_text(&mut self, text: &str) -> Result<()> {
        log_debug(&format!("Writing {} characters to clipboard", text.len()));

        self.clipboard
            .set_text(text)
            .context("Failed to write text to clipboard")?;

        log_info("Successfully wrote text to clipboard");
        Ok(())
    }
}

pub fn read_clipboard() -> Result<String> {
    let mut clipboard = ClipboardManager::new()?;
    clipboard.get_text()
}

pub fn write_clipboard(text: &str) -> Result<()> {
    let mut clipboard = ClipboardManager::new()?;
    clipboard.set_text(text)
}
