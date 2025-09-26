use anyhow::{Context, Result};
use base64::Engine;
use std::fs;
use std::path::Path;

use crate::logging::{log_debug, log_info};

/// Supported image formats for Gemini API
const SUPPORTED_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "heic", "pdf"];

/// Get MIME type from file extension
pub fn get_mime_type(file_path: &Path) -> Result<String> {
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_lowercase())
        .context("File has no extension")?;

    let mime_type = match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "heic" => "image/heic",
        "pdf" => "application/pdf",
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported file format: {}. Supported formats: {}",
                extension,
                SUPPORTED_EXTENSIONS.join(", ")
            ));
        }
    };

    Ok(mime_type.to_string())
}

/// Validate that the file exists and has a supported format
pub fn validate_image_file(file_path: &str) -> Result<()> {
    let path = Path::new(file_path);

    if !path.exists() {
        return Err(anyhow::anyhow!("Image file not found: {}", file_path));
    }

    if !path.is_file() {
        return Err(anyhow::anyhow!("Path is not a file: {}", file_path));
    }

    // Check file extension
    get_mime_type(path)?;

    log_debug(&format!("Validated image file: {}", file_path));
    Ok(())
}

/// Read image file and encode as base64
pub fn read_image_as_base64(file_path: &str) -> Result<String> {
    log_info(&format!("Reading image file: {}", file_path));

    let image_data =
        fs::read(file_path).with_context(|| format!("Failed to read image file: {}", file_path))?;

    let base64_data = base64::engine::general_purpose::STANDARD.encode(&image_data);

    log_info(&format!(
        "Successfully encoded image: {} bytes -> {} base64 chars",
        image_data.len(),
        base64_data.len()
    ));

    Ok(base64_data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::NamedTempFile;

    #[test]
    fn test_get_mime_type() {
        assert_eq!(get_mime_type(Path::new("test.jpg")).unwrap(), "image/jpeg");
        assert_eq!(get_mime_type(Path::new("test.jpeg")).unwrap(), "image/jpeg");
        assert_eq!(get_mime_type(Path::new("test.png")).unwrap(), "image/png");
        assert_eq!(get_mime_type(Path::new("test.webp")).unwrap(), "image/webp");
        assert_eq!(get_mime_type(Path::new("test.heic")).unwrap(), "image/heic");
        assert_eq!(
            get_mime_type(Path::new("test.pdf")).unwrap(),
            "application/pdf"
        );

        assert!(get_mime_type(Path::new("test.txt")).is_err());
        assert!(get_mime_type(Path::new("test")).is_err());
    }

    #[test]
    fn test_validate_image_file() {
        // Test with non-existent file
        assert!(validate_image_file("nonexistent.jpg").is_err());
    }

    #[test]
    fn test_read_image_as_base64() -> Result<()> {
        // Create a temporary image file
        let temp_file = NamedTempFile::new()?;
        let test_data = b"fake image data";
        fs::write(temp_file.path(), test_data)?;

        // Read as base64
        let base64_result = read_image_as_base64(temp_file.path().to_str().unwrap())?;

        // Verify the result
        let expected = base64::engine::general_purpose::STANDARD.encode(test_data);
        assert_eq!(base64_result, expected);

        Ok(())
    }
}
