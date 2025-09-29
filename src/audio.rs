use anyhow::{Context, Result};
use crossterm::event::{poll, read, Event, KeyCode, KeyEventKind};
use regex::Regex;
use std::fs;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

use crate::logging::{log_debug, log_info};

/// Record audio using ffmpeg and return the path to the recorded file
pub fn record_audio() -> Result<String> {
    log_info("Starting audio recording...");

    // Generate unique filename for the recording
    let temp_dir = std::env::temp_dir();
    let audio_file = temp_dir.join("prompt.m4a");
    let audio_path = audio_file.to_string_lossy().to_string();

    log_debug(&format!("Recording to: {audio_path}"));

    // Check if ffmpeg is available
    check_ffmpeg_available()?;

    // Get audio device (from environment variable or auto-detect)
    let audio_device = get_audio_device()?;
    log_info(&format!("Using audio device: {audio_device}"));

    println!("🎙️  Recording audio... Press Enter to stop recording");
    io::stdout().flush().unwrap();

    // Start ffmpeg recording with captured stdout/stderr for logging
    let mut ffmpeg_process = Command::new("ffmpeg")
        .args([
            "-f",
            "dshow",
            "-i",
            &format!("audio={audio_device}"),
            "-acodec",
            "aac",
            "-b:a",
            "64k",
            "-y", // Overwrite output file
            &audio_path,
        ])
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

    // Wait for user to stop recording using crossterm for keyboard detection
    println!("Press Enter to stop recording...");

    // Use crossterm to detect keyboard input regardless of stdin state
    loop {
        // Check for keyboard input with a short timeout
        match poll(Duration::from_millis(100)) {
            Ok(true) => {
                // Event is available, read it
                match read() {
                    Ok(Event::Key(key_event)) => {
                        // Only handle key press events (not release)
                        if key_event.kind == KeyEventKind::Press && key_event.code == KeyCode::Enter
                        {
                            log_debug("Enter key pressed, stopping recording");
                            break;
                        }
                        // Ignore other key events
                        log_debug(&format!("Ignoring key event: {key_event:?}"));
                    }
                    Ok(_) => {
                        // Non-key event (mouse, resize, etc.), ignore
                        log_debug("Ignoring non-key event");
                    }
                    Err(e) => {
                        log_debug(&format!("Error reading event: {e}"));
                    }
                }
            }
            Ok(false) => {
                // No event available, continue
            }
            Err(e) => {
                log_debug(&format!("Error polling for events: {e}"));
            }
        }

        // Check if the ffmpeg process is still running
        match ffmpeg_process.try_wait() {
            Ok(Some(_)) => {
                // Process has exited (likely due to error)
                log_debug("ffmpeg process exited unexpectedly");
                break;
            }
            Ok(None) => {
                // Process is still running, continue
            }
            Err(_) => {
                // Error checking status, break
                log_debug("Error checking ffmpeg status");
                break;
            }
        }
    }

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
    println!("✅ Audio recording complete!");

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
        if !device.trim().is_empty() {
            log_info(&format!(
                "Using audio device from GIA_AUDIO_DEVICE: {device}"
            ));
            return Ok(device);
        }
    }

    // Fall back to auto-detection
    log_debug("GIA_AUDIO_DEVICE not set, auto-detecting audio device");
    get_default_audio_device()
}

/// Get the default audio input device on Windows
fn get_default_audio_device() -> Result<String> {
    log_debug("Getting default audio device...");

    // Try to list audio devices using ffmpeg
    let output = Command::new("ffmpeg")
        .args(["-f", "dshow", "-list_devices", "true", "-i", "dummy"])
        .output()
        .context("Failed to list audio devices with ffmpeg")?;

    let stderr = String::from_utf8_lossy(&output.stderr);

    // Parse the ffmpeg output to find audio devices
    // Look for lines like: [dshow @ 000001fa49e956c0] "Device Name" (audio)
    let device_regex = Regex::new(r#"\[dshow +@ +[^\]]+\] +[# "]([^"']+)["'] +\(audio\)"#)
        .context("Failed to compile audio device regex")?;

    for line in stderr.lines() {
        log_info(line);
        if let Some(captures) = device_regex.captures(line) {
            if let Some(device_name) = captures.get(1) {
                let device_name = device_name.as_str();
                log_debug(&format!("Found audio device: {device_name}"));
                return Ok(device_name.to_string());
            }
        }
    }

    // Fallback to common device names if no devices found
    log_debug("No audio devices found, using fallback");
    Ok("Microphone".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_check_ffmpeg_available() {
        // This test might fail in CI environments without ffmpeg
        // but it's useful for local development
        let result = check_ffmpeg_available();

        // We don't assert success/failure since ffmpeg availability varies
        // Just ensure the function doesn't panic
        match result {
            Ok(()) => println!("ffmpeg is available"),
            Err(e) => println!("ffmpeg not available: {e}"),
        }
    }

    #[test]
    fn test_get_audio_device_with_env_var() {
        // Set environment variable
        std::env::set_var("GIA_AUDIO_DEVICE", "Test Microphone");

        let result = get_audio_device();

        // Clean up
        std::env::remove_var("GIA_AUDIO_DEVICE");

        // Verify result
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), "Test Microphone");
    }

    #[test]
    fn test_get_audio_device_without_env_var() {
        // Ensure env var is not set
        std::env::remove_var("GIA_AUDIO_DEVICE");

        let result = get_audio_device();

        // Should fall back to auto-detection (might fail in CI, but shouldn't panic)
        // We just verify it doesn't panic and returns a Result
        match result {
            Ok(device) => println!("Auto-detected device: {device}"),
            Err(e) => println!("Auto-detection failed (expected in CI): {e}"),
        }
    }
}
