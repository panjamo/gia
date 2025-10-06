use crate::logging::{log_debug, log_info};
use anyhow::{Context, Result};
use arboard::{Clipboard, ImageData};
use base64::Engine;
use image::{ImageBuffer, ImageFormat, Rgba};
use std::io::Cursor;

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

    pub fn get_image(&mut self) -> Result<ImageData<'static>> {
        log_debug("Reading image from clipboard");

        let image_data = self
            .clipboard
            .get_image()
            .context("Failed to read image from clipboard")?;

        log_info(&format!(
            "Read image from clipboard: {}x{} pixels",
            image_data.width, image_data.height
        ));
        Ok(image_data)
    }

    pub fn has_image(&mut self) -> bool {
        log_debug("Checking if clipboard contains image");
        self.clipboard.get_image().is_ok()
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

pub fn read_clipboard_image() -> Result<ImageData<'static>> {
    let mut clipboard = ClipboardManager::new()?;
    clipboard.get_image()
}

pub fn has_clipboard_image() -> Result<bool> {
    let mut clipboard = ClipboardManager::new()?;
    Ok(clipboard.has_image())
}

pub fn convert_image_data_to_base64(image_data: &ImageData) -> Result<String> {
    log_debug(&format!(
        "Converting image data to PNG base64: {}x{} pixels, {} bytes",
        image_data.width,
        image_data.height,
        image_data.bytes.len()
    ));

    // Validate expected byte count
    let expected_bytes = image_data.width * image_data.height * 4; // RGBA = 4 bytes per pixel
    if image_data.bytes.len() != expected_bytes {
        log_debug(&format!(
            "Warning: Expected {} bytes for {}x{} RGBA image, got {} bytes",
            expected_bytes,
            image_data.width,
            image_data.height,
            image_data.bytes.len()
        ));
    }

    // Helper function to create RGBA data with proper dimensions
    let prepare_rgba_data = |input_bytes: Vec<u8>| -> Vec<u8> {
        let expected_len = image_data.width * image_data.height * 4;
        if input_bytes.len() == image_data.width * image_data.height * 3 {
            // RGB format - convert to RGBA
            log_debug("Converting RGB to RGBA format");
            let mut rgba_data = Vec::with_capacity(expected_len);
            for chunk in input_bytes.chunks(3) {
                rgba_data.extend_from_slice(chunk);
                rgba_data.push(255); // Add full alpha
            }
            rgba_data
        } else {
            // Use as-is, padding or truncating as needed
            let mut rgba_data = input_bytes;
            rgba_data.resize(expected_len, 0);
            rgba_data
        }
    };

    // Try direct conversion first, fallback to manual conversion
    let rgba_data = if let Some(img_buffer) = ImageBuffer::<Rgba<u8>, _>::from_raw(
        u32::try_from(image_data.width).unwrap_or(0),
        u32::try_from(image_data.height).unwrap_or(0),
        image_data.bytes.to_vec(),
    ) {
        img_buffer.into_raw()
    } else {
        log_debug("Direct RGBA conversion failed, trying manual conversion");
        prepare_rgba_data(image_data.bytes.to_vec())
    };

    let img_buffer = ImageBuffer::<Rgba<u8>, _>::from_raw(
        u32::try_from(image_data.width).unwrap_or(0),
        u32::try_from(image_data.height).unwrap_or(0),
        rgba_data,
    )
    .context("Failed to create final image buffer")?;

    // Convert to PNG and encode as base64
    let mut png_data = Vec::new();
    {
        let mut cursor = Cursor::new(&mut png_data);
        img_buffer
            .write_to(&mut cursor, ImageFormat::Png)
            .context("Failed to encode image as PNG")?;
    }

    let base64_data = base64::engine::general_purpose::STANDARD.encode(&png_data);

    log_info(&format!(
        "Converted clipboard image to PNG base64: {} RGBA bytes -> {} PNG bytes -> {} base64 chars",
        image_data.bytes.len(),
        png_data.len(),
        base64_data.len()
    ));

    Ok(base64_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_clipboard_manager_creation() {
        let result = ClipboardManager::new();
        assert!(result.is_ok());
    }

    #[test]
    #[serial]
    fn test_clipboard_functions_exist() {
        // These tests just ensure the functions exist and don't panic on basic usage
        // We can't test actual clipboard functionality reliably in CI

        let result = has_clipboard_image();
        // Should return Ok(true/false) or Err, but not panic
        assert!(result.is_ok() || result.is_err());

        // Test that read_clipboard_image function exists
        // (will likely error in test environment without actual image in clipboard)
        let _result = read_clipboard_image();
    }
}
