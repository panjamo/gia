use anyhow::{Context, Result};
use base64::Engine;
use std::fs;
use std::path::Path;

use crate::constants::MEDIA_EXTENSIONS;
use crate::logging::log_info;

/// Get MIME type from file extension
pub fn get_mime_type(file_path: &Path) -> Result<String> {
    let extension = file_path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(str::to_lowercase)
        .context("File has no extension")?;

    let mime_type = match extension.as_str() {
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "webp" => "image/webp",
        "heic" => "image/heic",
        "pdf" => "application/pdf",
        "ogg" | "opus" => "audio/ogg",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "mp4" => "video/mp4",
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported file format: {}. Supported formats: {}",
                extension,
                MEDIA_EXTENSIONS.join(", ")
            ));
        }
    };

    Ok(mime_type.to_string())
}

/// Read media file and encode as base64
pub fn read_media_as_base64(file_path: &str) -> Result<String> {
    log_info(&format!("Reading media file: {file_path}"));

    let media_data =
        fs::read(file_path).with_context(|| format!("Failed to read media file: {file_path}"))?;

    let base64_data = base64::engine::general_purpose::STANDARD.encode(&media_data);

    log_info(&format!(
        "Successfully encoded media: {} bytes -> {} base64 chars",
        media_data.len(),
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
        assert_eq!(get_mime_type(Path::new("test.ogg")).unwrap(), "audio/ogg");
        assert_eq!(get_mime_type(Path::new("test.mp3")).unwrap(), "audio/mpeg");
        assert_eq!(get_mime_type(Path::new("test.m4a")).unwrap(), "audio/mp4");
        assert_eq!(get_mime_type(Path::new("test.mp4")).unwrap(), "video/mp4");

        assert!(get_mime_type(Path::new("test.txt")).is_err());
        assert!(get_mime_type(Path::new("test")).is_err());
    }

    #[test]
    fn test_read_media_as_base64() -> Result<()> {
        // Create a temporary media file
        let temp_file = NamedTempFile::new()?;
        let test_data = b"fake media data";
        fs::write(temp_file.path(), test_data)?;

        // Read as base64
        let base64_result = read_media_as_base64(temp_file.path().to_str().unwrap())?;

        // Verify the result
        let expected = base64::engine::general_purpose::STANDARD.encode(test_data);
        assert_eq!(base64_result, expected);

        Ok(())
    }

    #[test]
    fn test_get_mime_type_case_insensitive() {
        assert_eq!(get_mime_type(Path::new("test.JPG")).unwrap(), "image/jpeg");
        assert_eq!(get_mime_type(Path::new("test.PNG")).unwrap(), "image/png");
        assert_eq!(get_mime_type(Path::new("test.Mp3")).unwrap(), "audio/mpeg");
    }

    #[test]
    fn test_get_mime_type_audio_formats() {
        assert_eq!(get_mime_type(Path::new("file.ogg")).unwrap(), "audio/ogg");
        assert_eq!(get_mime_type(Path::new("file.opus")).unwrap(), "audio/ogg");
        assert_eq!(get_mime_type(Path::new("file.mp3")).unwrap(), "audio/mpeg");
        assert_eq!(get_mime_type(Path::new("file.m4a")).unwrap(), "audio/mp4");
    }

    #[test]
    fn test_get_mime_type_video_format() {
        assert_eq!(get_mime_type(Path::new("file.mp4")).unwrap(), "video/mp4");
    }

    #[test]
    fn test_get_mime_type_no_extension() {
        let result = get_mime_type(Path::new("file_without_extension"));
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("no extension"));
    }

    #[test]
    fn test_read_media_as_base64_empty_file() -> Result<()> {
        let temp_file = NamedTempFile::with_suffix(".jpg")?;
        // Create empty file
        fs::write(temp_file.path(), b"")?;

        let result = read_media_as_base64(temp_file.path().to_str().unwrap())?;

        // Empty file should produce empty base64 string
        assert_eq!(result, "");

        Ok(())
    }
}
