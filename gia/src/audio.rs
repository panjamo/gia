use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait};
use native_dialog::MessageDialog;
use regex::Regex;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::logging::{log_debug, log_info};

// Whitelist of allowed audio device characters to prevent command injection
fn validate_audio_device(device: &str) -> Result<String> {
    // Block dangerous shell metacharacters while allowing legitimate device name characters
    let dangerous_chars = [
        '&', '|', ';', '`', '$', '>', '<', '*', '?', '\\', '/', '"', '\'',
    ];

    if device.len() > 200 {
        return Err(anyhow::anyhow!(
            "Audio device name too long (max 200 characters)"
        ));
    }

    // Check for dangerous characters that could enable command injection
    for ch in dangerous_chars {
        if device.contains(ch) {
            log_debug(&format!(
                "Device validation failed for: '{}' - contains dangerous character: '{}'",
                device, ch
            ));
            return Err(anyhow::anyhow!(
                "Audio device name contains dangerous characters"
            ));
        }
    }

    Ok(device.to_string())
}

/// Get the default audio input device using cpal
/// Returns the device name if detected, None otherwise
fn get_cpal_default_device() -> Option<String> {
    log_debug("Detecting default audio device using cpal...");

    let host = cpal::default_host();
    if let Some(device) = host.default_input_device() {
        if let Ok(name) = device.name() {
            log_debug(&format!("cpal detected default input device: {}", name));
            return Some(name);
        } else {
            log_debug("cpal detected default input device but name unavailable");
        }
    } else {
        log_debug("cpal found no default input device");
    }
    None
}

/// Record audio using ffmpeg and return the path to the recorded file
pub fn record_audio() -> Result<String> {
    // Kill any existing ffmpeg processes that might interfere with recording
    log_debug("Checking for and terminating any existing ffmpeg processes...");

    #[cfg(target_os = "windows")]
    {
        let _ = Command::new("taskkill")
            .args(["/F", "/IM", "ffmpeg.exe"])
            .output();
    }

    #[cfg(any(target_os = "linux", target_os = "macos"))]
    {
        let _ = Command::new("pkill").args(["-f", "ffmpeg"]).output();
    }

    // Generate unique filename for the recording
    let temp_dir = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let audio_file = temp_dir.join(format!("{timestamp}-prompt.opus"));
    let audio_path = audio_file.to_string_lossy().to_string();

    log_debug(&format!("Recording to: {audio_path}"));

    // Check if ffmpeg is available
    check_ffmpeg_available()?;

    // Get and validate audio device
    let audio_device = get_audio_device()?;
    let validated_device =
        validate_audio_device(&audio_device).context("Invalid audio device name")?;

    log_info(&format!(
        "Starting audio recording with device: {validated_device}"
    ));
    eprintln!("🎙️  Recording audio from device: {validated_device}");

    // Start ffmpeg recording with captured stdout/stderr for logging
    // Use platform-specific audio input format
    let mut ffmpeg_cmd = Command::new("ffmpeg");

    #[cfg(target_os = "macos")]
    {
        // Support both numeric index (e.g., "0") and device name (e.g., "Built-in Microphone")
        // Use validated_device to ensure we only use sanitized input
        let device_arg = if validated_device.chars().all(|c| c.is_ascii_digit()) {
            // Numeric index: use existing :N format for backward compatibility
            format!(":{}", validated_device)
        } else {
            // Device name: use :device_name format
            format!(":{}", validated_device)
        };

        ffmpeg_cmd.args([
            "-f",
            "avfoundation",
            "-i",
            &device_arg,
            "-c:a",
            "libopus",
            "-b:a",
            "32k",
            "-y", // Overwrite output file
            &audio_path,
        ]);
    }

    #[cfg(target_os = "windows")]
    {
        ffmpeg_cmd.args([
            "-f",
            "dshow",
            "-i",
            &format!("audio={}", validated_device),
            "-c:a",
            "libopus",
            "-b:a",
            "32k",
            "-y", // Overwrite output file
            &audio_path,
        ]);
    }

    #[cfg(target_os = "linux")]
    {
        ffmpeg_cmd.args([
            "-f",
            "pulse",
            "-i",
            &audio_device,
            "-c:a",
            "libopus",
            "-b:a",
            "32k",
            "-y", // Overwrite output file
            &audio_path,
        ]);
    }

    let mut ffmpeg_process = ffmpeg_cmd
        .stdin(Stdio::piped())
        .stdout(Stdio::piped()) // Capture stdout for logging
        .stderr(Stdio::piped()) // Capture stderr for logging
        .spawn()
        .context("Failed to start ffmpeg recording")?;

    // Spawn threads to handle stdout and stderr logging
    let stdout = ffmpeg_process.stdout.take().unwrap();
    let stderr = ffmpeg_process.stderr.take().unwrap();

    let _stdout_handle = thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            log_info(&format!("ffmpeg: {line}"));
        }
    });

    let _stderr_handle = thread::spawn(move || {
        let reader = BufReader::new(stderr);
        for line in reader.lines().map_while(Result::ok) {
            log_debug(&format!("ffmpeg: {line}"));
        }
    });

    // Wait 1 second and check if ffmpeg process is still running
    log_debug("Waiting 1 second before showing dialog...");
    thread::sleep(Duration::from_secs(1));

    match ffmpeg_process.try_wait() {
        Ok(Some(status)) => {
            // Process has already exited
            log_debug(&format!(
                "ffmpeg process exited early with status: {status}"
            ));

            // Show error dialog to user
            let error_msg = format!(
                "Audio recording failed!\n\nffmpeg process exited unexpectedly.\nExit code: {}\n\nPlease check:\n- Audio device is available\n- ffmpeg is properly installed\n- GIA_AUDIO_DEVICE is set correctly (if used)",
                status
                    .code()
                    .map_or("unknown".to_string(), |c| c.to_string())
            );

            let _ = MessageDialog::new()
                .set_title("Recording Error")
                .set_text(&error_msg)
                .set_type(native_dialog::MessageType::Error)
                .show_alert();

            return Err(anyhow::anyhow!(
                "Audio recording failed - ffmpeg process exited unexpectedly"
            ));
        }
        Ok(None) => {
            // Process is still running, continue
            log_debug("ffmpeg process is running");
        }
        Err(e) => {
            log_debug(&format!("Error checking ffmpeg status: {e}"));
            return Err(anyhow::anyhow!(
                "Failed to check ffmpeg process status: {e}"
            ));
        }
    }

    // Show message dialog to stop recording
    log_debug("Showing message dialog to stop recording");
    let dialog_text = format!(
        "🎙️  Recording in progress from device:\n{}\n\nClick Yes to stop and continue, or No to abort.",
        validated_device
    );
    let user_confirmed = MessageDialog::new()
        .set_title("Stop Recording")
        .set_text(&dialog_text)
        .set_type(native_dialog::MessageType::Info)
        .show_confirm()
        .context("Failed to show recording dialog")?;

    if !user_confirmed {
        log_debug("User pressed Cancel, aborting");

        // Kill ffmpeg process
        let _ = ffmpeg_process.kill();
        let _ = ffmpeg_process.wait();

        return Err(anyhow::anyhow!("Recording cancelled by user"));
    }

    log_debug("User clicked OK, stopping recording");

    // Stop recording by terminating ffmpeg
    log_debug("Stopping audio recording...");

    // Send 'q' to ffmpeg's stdin to stop recording gracefully
    if let Some(stdin) = ffmpeg_process.stdin.as_mut() {
        let _ = stdin.write_all(b"q\n");
        let _ = stdin.flush();
    }

    // Busy wait for process to exit gracefully with timeout
    let start_time = std::time::Instant::now();
    let timeout = std::time::Duration::from_millis(5000); // 5 second timeout
    let poll_interval = std::time::Duration::from_millis(50); // Check every 50ms

    loop {
        match ffmpeg_process.try_wait() {
            Ok(Some(_)) => {
                log_debug("ffmpeg exited gracefully");
                break;
            }
            Ok(None) => {
                // Process is still running, check if we've timed out
                if start_time.elapsed() > timeout {
                    log_debug("ffmpeg graceful exit timeout, force killing...");
                    let _ = ffmpeg_process.kill();
                    break;
                }
                // Wait a bit before checking again
                std::thread::sleep(poll_interval);
            }
            Err(_) => {
                // Error checking status, try to kill anyway
                log_debug("Error checking ffmpeg status, force killing...");
                let _ = ffmpeg_process.kill();
                break;
            }
        }
    }

    // Wait for the process to actually exit
    let _ = ffmpeg_process.wait();

    // Verify the file was created
    if !Path::new(&audio_path).exists() {
        return Err(anyhow::anyhow!(
            "Audio recording failed - output file not found"
        ));
    }

    let file_size = fs::metadata(&audio_path)
        .context("Failed to get audio file metadata")?
        .len();

    if file_size == 0 {
        return Err(anyhow::anyhow!(
            "Audio recording failed - output file is empty"
        ));
    }

    log_info(&format!(
        "✅ Audio recorded successfully: {file_size} bytes"
    ));
    eprintln!("✅ Audio recording complete!");

    Ok(audio_path)
}

/// Check if ffmpeg is available in the system
fn check_ffmpeg_available() -> Result<()> {
    log_debug("Checking if ffmpeg is available...");

    let output = Command::new("ffmpeg")
        .args(["-version"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();

    match output {
        Ok(status) if status.success() => {
            log_debug("ffmpeg is available");
            Ok(())
        }
        _ => Err(anyhow::anyhow!(
            "ffmpeg is not available. Please install ffmpeg and ensure it's in your PATH.\n\
             Download from: https://ffmpeg.org/download.html"
        )),
    }
}

/// Get the audio device (from environment variable or auto-detect)
fn get_audio_device() -> Result<String> {
    // Check for environment variable first
    if let Ok(device) = std::env::var("GIA_AUDIO_DEVICE") {
        let trimmed = device.trim();
        if !trimmed.is_empty() {
            log_info(&format!(
                "Using audio device from GIA_AUDIO_DEVICE: {trimmed}"
            ));
            return Ok(trimmed.to_string());
        }
    }

    // Fall back to auto-detection
    log_debug("GIA_AUDIO_DEVICE not set, auto-detecting audio device");

    // Get CPAL default device once, then pass to platform-specific detection
    let cpal_device = get_cpal_default_device();
    get_default_audio_device(cpal_device)
}

/// Get the default audio input device for the current platform
/// Takes an optional CPAL-detected device name to match against platform-specific devices
fn get_default_audio_device(cpal_device: Option<String>) -> Result<String> {
    log_debug("Getting default audio device...");

    #[cfg(target_os = "macos")]
    {
        // macOS behavior note:
        // - Auto-detection now returns device NAME (e.g., "Built-in Microphone") instead of INDEX
        // - Backward compatible: numeric indices (e.g., "0") still work via GIA_AUDIO_DEVICE
        // - This matches CPAL device names for better cross-platform consistency

        // Try to list audio devices using ffmpeg with avfoundation
        let output = Command::new("ffmpeg")
            .args(["-f", "avfoundation", "-list_devices", "true", "-i", ""])
            .output()
            .context("Failed to list audio devices with ffmpeg")?;

        let stderr = String::from_utf8_lossy(&output.stderr);

        // Parse the ffmpeg output to find audio devices
        // The output format is:
        // [AVFoundation indev @ ...] AVFoundation video devices:
        // [AVFoundation indev @ ...] [0] FaceTime HD Camera
        // [AVFoundation indev @ ...] AVFoundation audio devices:
        // [AVFoundation indev @ ...] [0] Built-in Microphone

        let mut in_audio_section = false;
        let device_regex = Regex::new(r#"\[AVFoundation[^\]]*\] \[(\d+)\] (.+)"#)
            .context("Failed to compile audio device regex")?;

        let mut ffmpeg_devices: Vec<(String, String)> = Vec::new(); // (index, name)

        for line in stderr.lines() {
            log_debug(line);

            // Check if we've entered the audio devices section
            if line.contains("AVFoundation audio devices:") {
                in_audio_section = true;
                continue;
            }

            // If we're in the audio section, look for device entries
            if in_audio_section
                && let Some(captures) = device_regex.captures(line)
                && let (Some(device_idx), Some(device_name)) = (captures.get(1), captures.get(2))
            {
                let device_name = device_name.as_str().to_string();
                let device_idx = device_idx.as_str().to_string();
                log_debug(&format!(
                    "Found audio device [{}]: {}",
                    device_idx, device_name
                ));
                ffmpeg_devices.push((device_idx, device_name));
            }
        }

        // If CPAL detected a device, try to match it with FFmpeg devices
        if let Some(cpal_name) = cpal_device {
            log_debug(&format!(
                "Trying to match CPAL device '{}' with FFmpeg devices",
                cpal_name
            ));
            for (_, ffmpeg_name) in &ffmpeg_devices {
                if ffmpeg_name == &cpal_name {
                    log_info(&format!("Matched CPAL device to FFmpeg: {}", cpal_name));
                    return Ok(cpal_name);
                }
            }
            log_debug("No exact match found between CPAL and FFmpeg devices");
        }

        // Return first device name if available
        if let Some((_, first_device_name)) = ffmpeg_devices.first() {
            return Ok(first_device_name.clone());
        }

        // Fallback to common device name if no devices found
        log_debug("No audio devices found, using fallback");
        Ok("Built-in Microphone".to_string())
    }

    #[cfg(target_os = "windows")]
    {
        // Try to list audio devices using ffmpeg with dshow
        let output = Command::new("ffmpeg")
            .args(["-f", "dshow", "-list_devices", "true", "-i", "dummy"])
            .output()
            .context("Failed to list audio devices with ffmpeg")?;

        let stderr = String::from_utf8_lossy(&output.stderr);

        // Parse the ffmpeg output to find audio devices
        // Look for lines like: [dshow @ 000001fa49e956c0] "Device Name" (audio)
        let device_regex = Regex::new(r#"\[dshow +@ +[^\]]+\] +[# "]([^"']+)["'] +\(audio\)"#)
            .context("Failed to compile audio device regex")?;

        let mut ffmpeg_devices = Vec::new();
        for line in stderr.lines() {
            log_debug(line);
            if let Some(captures) = device_regex.captures(line)
                && let Some(device_name) = captures.get(1)
            {
                let device_name = device_name.as_str();
                log_debug(&format!("Found audio device: {device_name}"));
                ffmpeg_devices.push(device_name.to_string());
            }
        }

        // If CPAL detected a device, try to match it exactly with FFmpeg devices
        if let Some(cpal_name) = cpal_device {
            log_debug(&format!(
                "Trying to match CPAL device '{}' with FFmpeg devices",
                cpal_name
            ));
            for ffmpeg_device in &ffmpeg_devices {
                if ffmpeg_device == &cpal_name {
                    log_info(&format!("Matched CPAL device to FFmpeg: {}", cpal_name));
                    return Ok(cpal_name);
                }
            }
            log_debug("No exact match found between CPAL and FFmpeg devices");
        }

        // Return first FFmpeg device if available
        if let Some(first_device) = ffmpeg_devices.first() {
            return Ok(first_device.clone());
        }

        // Fallback to common device names if no devices found
        log_debug("No audio devices found, using fallback");
        Ok("Microphone".to_string())
    }

    #[cfg(target_os = "linux")]
    {
        // Note: CPAL device is passed in but not used on Linux
        // PulseAudio's "default" is more reliable than trying to match CPAL device names

        // Try to list audio devices using ffmpeg with pulse
        let output = Command::new("ffmpeg")
            .args(["-f", "pulse", "-list_devices", "true", "-i", ""])
            .output()
            .context("Failed to list audio devices with ffmpeg")?;

        let stderr = String::from_utf8_lossy(&output.stderr);

        // Parse the ffmpeg output to find audio devices
        for line in stderr.lines() {
            log_debug(line);
            // PulseAudio devices are typically listed as "default" or with specific names
            if line.contains("Source") || line.contains("default") {
                log_debug("Found default audio source");
                return Ok("default".to_string());
            }
        }

        // Fallback to "default" if no devices found
        log_debug("No audio devices found, using fallback");
        Ok("default".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    fn test_validate_audio_device_valid() {
        let result = validate_audio_device("Test Microphone");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Test Microphone");
    }

    #[test]
    fn test_validate_audio_device_command_injection() {
        let result = validate_audio_device("Test; rm -rf /");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_audio_device_pipe_injection() {
        let result = validate_audio_device("Test | malicious");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_audio_device_ampersand_injection() {
        let result = validate_audio_device("Test & evil");
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_audio_device_too_long() {
        let long_name = "a".repeat(201);
        let result = validate_audio_device(&long_name);
        assert!(result.is_err());
    }

    #[test]
    fn test_validate_audio_device_with_brackets() {
        let result = validate_audio_device("Test Microphone [USB]");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Test Microphone [USB]");
    }

    #[test]
    fn test_validate_audio_device_with_parentheses() {
        let result = validate_audio_device("Test Microphone (High Definition)");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Test Microphone (High Definition)");
    }

    #[test]
    fn test_validate_audio_device_real_device_name() {
        let result =
            validate_audio_device("Headset Microphone (2- Plantronics Blackwire 5220 Series)");
        assert!(result.is_ok());
        assert_eq!(
            result.unwrap(),
            "Headset Microphone (2- Plantronics Blackwire 5220 Series)"
        );
    }

    #[test]
    fn test_validate_audio_device_with_numbers_and_dashes() {
        let result = validate_audio_device("Audio Device 123-456");
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Audio Device 123-456");
    }

    #[test]
    fn test_check_ffmpeg_available() {
        let result = check_ffmpeg_available();
        match result {
            Ok(()) => println!("ffmpeg is available"),
            Err(e) => println!("ffmpeg not available: {e}"),
        }
    }

    #[test]
    #[serial]
    fn test_get_audio_device_with_env_var() {
        unsafe { std::env::set_var("GIA_AUDIO_DEVICE", "Test Microphone") };

        let result = get_audio_device();

        unsafe { std::env::remove_var("GIA_AUDIO_DEVICE") };

        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Test Microphone");
    }

    #[test]
    #[serial]
    fn test_get_audio_device_without_env_var() {
        unsafe { std::env::remove_var("GIA_AUDIO_DEVICE") };

        let result = get_audio_device();

        match result {
            Ok(device) => println!("Auto-detected device: {device}"),
            Err(e) => println!("Auto-detection failed (expected in CI): {e}"),
        }
    }

    #[test]
    fn test_get_cpal_default_device() {
        // This test ensures the function doesn't panic and returns valid data
        let result = get_cpal_default_device();

        match result {
            Some(device) => {
                println!("CPAL detected device: {}", device);
                // Verify the device name is not empty
                assert!(!device.is_empty(), "Device name should not be empty");
                // Verify the device name doesn't contain dangerous characters
                assert!(
                    validate_audio_device(&device).is_ok(),
                    "Device name should pass validation: {}",
                    device
                );
            }
            None => {
                println!("CPAL found no default device (may be expected in CI environments)");
                // This is acceptable - some systems may not have audio devices
            }
        }
    }
}
