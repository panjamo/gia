use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use native_dialog::MessageDialog;
use std::fs;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::logging::{log_debug, log_info};

/// Record audio natively using cpal (fast recording to WAV, then quick ogg-opus conversion)
/// Returns the path to the recorded Opus file
pub fn record_audio_native() -> Result<String> {
    log_debug("Starting native audio recording with cpal");

    // Generate unique filenames for WAV and Opus
    let temp_dir = std::env::temp_dir();
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let wav_file = temp_dir.join(format!("{timestamp}-prompt.wav"));
    let opus_file = temp_dir.join(format!("{timestamp}-prompt.opus"));
    let wav_path = wav_file.to_string_lossy().to_string();
    let opus_path = opus_file.to_string_lossy().to_string();

    log_debug(&format!("Recording to: {wav_path}"));

    // Get audio device and configuration
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| anyhow::anyhow!("No default input device available"))?;

    let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
    log_info(&format!("Using audio device: {device_name}"));
    eprintln!("🎙️  Recording audio from device: {device_name}");

    let config = device
        .default_input_config()
        .context("Failed to get default input config")?;

    log_debug(&format!(
        "Audio config - Sample rate: {}, Channels: {}, Format: {:?}",
        config.sample_rate().0,
        config.channels(),
        config.sample_format()
    ));

    // Create WAV writer
    let spec = hound::WavSpec {
        channels: config.channels(),
        sample_rate: config.sample_rate().0,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let writer = Arc::new(Mutex::new(
        hound::WavWriter::create(&wav_path, spec).context("Failed to create WAV writer")?,
    ));
    let writer_clone = Arc::clone(&writer);

    log_debug("WAV writer created successfully");

    // Flag to signal recording stop
    let recording = Arc::new(Mutex::new(true));
    let recording_clone = Arc::clone(&recording);

    // Build input stream based on sample format - write directly to WAV
    let stream = match config.sample_format() {
        cpal::SampleFormat::F32 => device.build_input_stream(
            &config.into(),
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if *recording_clone.lock().unwrap() {
                    let mut writer = writer_clone.lock().unwrap();
                    for &sample in data {
                        let _ = writer.write_sample((sample * 32767.0) as i16);
                    }
                }
            },
            |err| log_debug(&format!("Stream error: {err}")),
            None,
        ),
        cpal::SampleFormat::I16 => device.build_input_stream(
            &config.into(),
            move |data: &[i16], _: &cpal::InputCallbackInfo| {
                if *recording_clone.lock().unwrap() {
                    let mut writer = writer_clone.lock().unwrap();
                    for &sample in data {
                        let _ = writer.write_sample(sample);
                    }
                }
            },
            |err| log_debug(&format!("Stream error: {err}")),
            None,
        ),
        cpal::SampleFormat::U16 => device.build_input_stream(
            &config.into(),
            move |data: &[u16], _: &cpal::InputCallbackInfo| {
                if *recording_clone.lock().unwrap() {
                    let mut writer = writer_clone.lock().unwrap();
                    for &sample in data {
                        let _ = writer.write_sample((sample as i32 - 32768) as i16);
                    }
                }
            },
            |err| log_debug(&format!("Stream error: {err}")),
            None,
        ),
        _ => {
            return Err(anyhow::anyhow!(
                "Unsupported sample format: {:?}",
                config.sample_format()
            ));
        }
    }
    .context("Failed to build input stream")?;

    // Start recording
    stream.play().context("Failed to start audio stream")?;
    log_debug("Audio stream started");

    // Wait a bit for stream to initialize
    thread::sleep(Duration::from_millis(100));

    // Show message dialog to stop recording
    log_debug("Showing message dialog to stop recording");
    let dialog_text = format!(
        "🎙️  Recording in progress from device:\n{}\n\nClick Yes to stop and continue, or No to abort.",
        device_name
    );
    let user_confirmed = MessageDialog::new()
        .set_title("Stop Recording")
        .set_text(&dialog_text)
        .set_type(native_dialog::MessageType::Info)
        .show_confirm()
        .context("Failed to show recording dialog")?;

    if !user_confirmed {
        log_debug("User pressed Cancel, aborting");
        *recording.lock().unwrap() = false;
        drop(stream);
        return Err(anyhow::anyhow!("Recording cancelled by user"));
    }

    log_debug("User clicked OK, stopping recording");

    // Stop recording
    *recording.lock().unwrap() = false;
    drop(stream);

    // Finalize WAV file
    Arc::try_unwrap(writer)
        .map_err(|_| anyhow::anyhow!("Failed to unwrap WAV writer Arc"))?
        .into_inner()
        .map_err(|_| anyhow::anyhow!("Failed to unwrap WAV writer Mutex"))?
        .finalize()
        .context("Failed to finalize WAV file")?;

    log_debug("WAV file finalized");

    // Check WAV file size
    let wav_size = fs::metadata(&wav_path)
        .context("Failed to get WAV file metadata")?
        .len();

    if wav_size == 0 {
        return Err(anyhow::anyhow!("WAV file is empty - no audio recorded"));
    }

    log_info(&format!("✅ Recorded WAV file: {wav_size} bytes"));

    // Convert WAV to Opus using ogg-opus (fast since WAV is already recorded)
    log_debug("Converting WAV to Opus with ogg-opus...");
    eprintln!("🔄 Converting to Opus format...");

    // Read WAV file back using hound
    let mut reader =
        hound::WavReader::open(&wav_path).context("Failed to open WAV file for conversion")?;
    let wav_spec = reader.spec();

    // Read all samples as i16
    let audio_data: Vec<i16> = reader
        .samples::<i16>()
        .collect::<Result<Vec<i16>, _>>()
        .context("Failed to read audio samples from WAV file")?;

    // Encode to Opus based on sample rate and channel count
    let opus_data = match (wav_spec.sample_rate, wav_spec.channels) {
        (16000, 1) => ogg_opus::encode::<16000, 1>(&audio_data),
        (16000, 2) => ogg_opus::encode::<16000, 2>(&audio_data),
        (8000, 1) => ogg_opus::encode::<8000, 1>(&audio_data),
        (8000, 2) => ogg_opus::encode::<8000, 2>(&audio_data),
        (12000, 1) => ogg_opus::encode::<12000, 1>(&audio_data),
        (12000, 2) => ogg_opus::encode::<12000, 2>(&audio_data),
        (24000, 1) => ogg_opus::encode::<24000, 1>(&audio_data),
        (24000, 2) => ogg_opus::encode::<24000, 2>(&audio_data),
        (48000, 1) => ogg_opus::encode::<48000, 1>(&audio_data),
        (48000, 2) => ogg_opus::encode::<48000, 2>(&audio_data),
        _ => return Err(anyhow::anyhow!(
            "Unsupported WAV format: {} Hz, {} channels (supported: 8k/12k/16k/24k/48k Hz, 1-2 channels)",
            wav_spec.sample_rate,
            wav_spec.channels
        )),
    }.context("Failed to encode audio to Opus format")?;

    // Write Opus file
    fs::write(&opus_path, &opus_data).context("Failed to write Opus file")?;

    // Clean up WAV file
    let _ = fs::remove_file(&wav_path);

    // Verify Opus file
    let opus_size = opus_data.len() as u64;
    if opus_size == 0 {
        return Err(anyhow::anyhow!(
            "Opus conversion failed - output file is empty"
        ));
    }

    log_info(&format!("✅ Converted to Opus: {opus_size} bytes"));
    eprintln!("✅ Audio recording complete!");

    Ok(opus_path)
}

/// Record audio using native Rust implementation (cpal + ogg-opus)
/// No external dependencies required
pub fn record_audio() -> Result<String> {
    log_debug("Starting native audio recording...");
    record_audio_native()
}

#[cfg(test)]
mod tests {
    use super::*;

    // All FFmpeg-related tests have been removed
    // The audio recording now uses native Rust implementation only (cpal + ogg-opus)
}
