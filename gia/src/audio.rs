use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use native_dialog::MessageDialog;
use rubato::{
    Resampler, SincFixedIn, SincInterpolationParameters, SincInterpolationType, WindowFunction,
};
use std::fs;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use crate::logging::{log_debug, log_info};

/// Resample audio data to a target sample rate
/// Returns resampled data and the target sample rate
fn resample_audio(audio_data: Vec<i16>, from_rate: u32, channels: u16) -> Result<(Vec<i16>, u32)> {
    // Supported rates by ogg-opus in order of preference
    const SUPPORTED_RATES: [u32; 5] = [48000, 24000, 16000, 12000, 8000];

    // If already supported, return as-is
    if SUPPORTED_RATES.contains(&from_rate) {
        return Ok((audio_data, from_rate));
    }

    // Choose target rate (prefer 48000 Hz for quality)
    let to_rate = 48000u32;

    log_info(&format!(
        "Resampling audio from {} Hz to {} Hz ({} channels)",
        from_rate, to_rate, channels
    ));
    eprintln!(
        "🔄 Resampling audio from {} Hz to {} Hz...",
        from_rate, to_rate
    );

    // Convert i16 samples to f32 for resampling
    let mut samples_f32: Vec<Vec<f32>> = vec![Vec::new(); channels as usize];

    for (i, &sample) in audio_data.iter().enumerate() {
        let channel = i % channels as usize;
        samples_f32[channel].push(sample as f32 / 32768.0);
    }

    // Create resampler with high-quality settings
    let params = SincInterpolationParameters {
        sinc_len: 256,
        f_cutoff: 0.95,
        interpolation: SincInterpolationType::Linear,
        oversampling_factor: 256,
        window: WindowFunction::BlackmanHarris2,
    };

    let mut resampler = SincFixedIn::<f32>::new(
        to_rate as f64 / from_rate as f64,
        2.0,
        params,
        samples_f32[0].len(),
        channels as usize,
    )
    .context("Failed to create resampler")?;

    // Resample each channel
    let resampled = resampler
        .process(&samples_f32, None)
        .context("Failed to resample audio")?;

    // Interleave channels back to i16
    let mut output = Vec::new();
    let sample_count = resampled[0].len();

    for i in 0..sample_count {
        for (_channel, channel_data) in resampled.iter().enumerate().take(channels as usize) {
            let sample = (channel_data[i] * 32768.0).clamp(-32768.0, 32767.0) as i16;
            output.push(sample);
        }
    }

    log_info(&format!(
        "Resampling complete: {} samples -> {} samples",
        audio_data.len() / channels as usize,
        output.len() / channels as usize
    ));

    Ok((output, to_rate))
}

/// List all available audio input devices
pub fn list_audio_devices() -> Result<()> {
    let host = cpal::default_host();

    println!("Available audio input devices:");
    println!();

    // Get default device name once before the loop
    let default_device_name = host.default_input_device().and_then(|d| d.name().ok());

    // Print default device if available
    if let Some(ref name) = default_device_name {
        println!("  [DEFAULT] {}", name);
    }

    // List all input devices
    let devices = host
        .input_devices()
        .context("Failed to enumerate audio input devices")?;

    let mut count = 0;
    for device in devices {
        let device_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
        // Skip default device as we already printed it
        if let Some(ref default_name) = default_device_name
            && device_name == *default_name
        {
            continue;
        }
        println!("  {}", device_name);
        count += 1;
    }

    if count == 0 && default_device_name.is_none() {
        println!("  No audio input devices found");
    }

    println!();
    println!("Usage:");
    println!("  gia --audio-device \"Device Name\" --record-audio \"your prompt\"");
    println!("  GIA_AUDIO_DEVICE=\"Device Name\" gia --record-audio \"your prompt\"");

    Ok(())
}

/// Helper function to find a device by name
fn find_device_by_name(host: &cpal::Host, target_name: &str) -> Result<Option<cpal::Device>> {
    let devices = host
        .input_devices()
        .context("Failed to enumerate audio input devices")?;

    for device in devices {
        if let Ok(dev_name) = device.name()
            && dev_name == target_name
        {
            return Ok(Some(device));
        }
    }

    Ok(None)
}

/// Get the audio device to use based on priority: CLI param > env var > default
fn get_audio_device(device_name: Option<&str>) -> Result<cpal::Device> {
    let host = cpal::default_host();

    // Priority 1: CLI parameter
    if let Some(name) = device_name {
        log_debug(&format!(
            "Looking for audio device from CLI parameter: {}",
            name
        ));

        if let Some(device) = find_device_by_name(&host, name)? {
            log_info(&format!("Using audio device from CLI parameter: {}", name));
            return Ok(device);
        }

        return Err(anyhow::anyhow!(
            "Audio device '{}' not found. Use --list-audio-devices to see available devices.",
            name
        ));
    }

    // Priority 2: Environment variable
    if let Ok(env_device) = std::env::var("GIA_AUDIO_DEVICE") {
        log_debug(&format!(
            "Looking for audio device from GIA_AUDIO_DEVICE: {}",
            env_device
        ));

        if let Some(device) = find_device_by_name(&host, &env_device)? {
            log_info(&format!(
                "Using audio device from GIA_AUDIO_DEVICE: {}",
                env_device
            ));
            return Ok(device);
        }

        return Err(anyhow::anyhow!(
            "Audio device '{}' (from GIA_AUDIO_DEVICE) not found. Use --list-audio-devices to see available devices.",
            env_device
        ));
    }

    // Priority 3: Default device
    log_debug("Using default audio input device");
    host.default_input_device()
        .ok_or_else(|| anyhow::anyhow!("No default input device available"))
}

/// Record audio natively using cpal (fast recording to WAV, then quick ogg-opus conversion)
/// Returns the path to the recorded Opus file
pub fn record_audio_native(device_name: Option<&str>) -> Result<String> {
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

    // Get audio device and configuration (priority: CLI param > env var > default)
    let device = get_audio_device(device_name)?;

    let device_display_name = device.name().unwrap_or_else(|_| "Unknown".to_string());
    log_info(&format!("Using audio device: {device_display_name}"));
    eprintln!("🎙️  Recording audio from device: {device_display_name}");

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

    let writer = Arc::new(Mutex::new(Some(
        hound::WavWriter::create(&wav_path, spec).context("Failed to create WAV writer")?,
    )));
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
                if *recording_clone.lock().unwrap()
                    && let Some(ref mut writer) = *writer_clone.lock().unwrap()
                {
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
                if *recording_clone.lock().unwrap()
                    && let Some(ref mut writer) = *writer_clone.lock().unwrap()
                {
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
                if *recording_clone.lock().unwrap()
                    && let Some(ref mut writer) = *writer_clone.lock().unwrap()
                {
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
        device_display_name
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
    // Extract the writer from the Option without unwrapping the Arc (cpal pattern)
    writer
        .lock()
        .unwrap()
        .take()
        .ok_or_else(|| anyhow::anyhow!("WAV writer was already taken"))?
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

    // Resample if necessary (handles any sample rate -> supported rate)
    let (resampled_data, target_rate) =
        resample_audio(audio_data, wav_spec.sample_rate, wav_spec.channels)?;

    // Encode to Opus based on sample rate and channel count
    let opus_data = match (target_rate, wav_spec.channels) {
        (16000, 1) => ogg_opus::encode::<16000, 1>(&resampled_data),
        (16000, 2) => ogg_opus::encode::<16000, 2>(&resampled_data),
        (8000, 1) => ogg_opus::encode::<8000, 1>(&resampled_data),
        (8000, 2) => ogg_opus::encode::<8000, 2>(&resampled_data),
        (12000, 1) => ogg_opus::encode::<12000, 1>(&resampled_data),
        (12000, 2) => ogg_opus::encode::<12000, 2>(&resampled_data),
        (24000, 1) => ogg_opus::encode::<24000, 1>(&resampled_data),
        (24000, 2) => ogg_opus::encode::<24000, 2>(&resampled_data),
        (48000, 1) => ogg_opus::encode::<48000, 1>(&resampled_data),
        (48000, 2) => ogg_opus::encode::<48000, 2>(&resampled_data),
        _ => return Err(anyhow::anyhow!(
            "Unsupported WAV format after resampling: {} Hz, {} channels (supported: 8k/12k/16k/24k/48k Hz, 1-2 channels)",
            target_rate,
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
///
/// Priority for device selection:
/// 1. device_name parameter (from CLI --audio-device)
/// 2. GIA_AUDIO_DEVICE environment variable
/// 3. Default system audio input device
pub fn record_audio(device_name: Option<&str>) -> Result<String> {
    log_debug("Starting native audio recording...");
    record_audio_native(device_name)
}

#[cfg(test)]
mod tests {
    // All FFmpeg-related tests have been removed
    // The audio recording now uses native Rust implementation only (cpal + ogg-opus)
}
