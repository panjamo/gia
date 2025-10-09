use arboard::Clipboard;
use clap::Parser;
use eframe::egui;
use serde::Deserialize;
use std::fs;

use std::process::Command;
use std::sync::{Arc, Mutex};
use std::thread;

#[cfg(not(target_os = "macos"))]
use notify_rust::Notification;

/// Ollama API response for /api/tags
#[derive(Debug, Deserialize)]
struct OllamaTagsResponse {
    models: Vec<OllamaModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaModel {
    name: String,
}

const DEFAULT_OLLAMA_BASE_URL: &str = "http://localhost:11434";

/// GIA GUI - Graphical user interface for the GIA command-line tool
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Display only a spinner until the process is killed
    #[arg(short, long)]
    spinner: bool,
}

/// Fetch available Ollama models from local Ollama instance (blocking).
///
/// # Returns
/// Vector of model names in format "ollama::model-name", or empty vec on failure.
///
/// # Environment Variables
/// - `OLLAMA_API_BASE`: Custom Ollama server URL (default: http://localhost:11434)
///
/// # Errors
/// Returns empty vec on: invalid URL, network timeout (2s), or parse failure.
fn fetch_ollama_models() -> Vec<String> {
    let base_url =
        std::env::var("OLLAMA_API_BASE").unwrap_or_else(|_| DEFAULT_OLLAMA_BASE_URL.to_string());

    let base = match reqwest::Url::parse(&base_url) {
        Ok(url) => url,
        Err(e) => {
            eprintln!("Ollama: invalid base URL '{}': {}", base_url, e);
            return Vec::new();
        }
    };

    let url = match base.join("/api/tags") {
        Ok(url) => url,
        Err(e) => {
            eprintln!("Ollama: failed to construct API URL: {}", e);
            return Vec::new();
        }
    };

    let client = match reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(2))
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Ollama: failed to create HTTP client: {}", e);
            return Vec::new();
        }
    };

    match client.get(url).send() {
        Ok(response) => match response.json::<OllamaTagsResponse>() {
            Ok(data) => data
                .models
                .into_iter()
                .map(|m| format!("ollama::{}", m.name))
                .collect(),
            Err(e) => {
                eprintln!("Ollama: failed to parse response: {}", e);
                Vec::new()
            }
        },
        Err(e) => {
            eprintln!("Ollama: connection failed: {}", e);
            Vec::new()
        }
    }
}

/// Show a system notification when audio recording is complete
fn show_completion_notification() {
    #[cfg(target_os = "macos")]
    {
        // On macOS, use osascript to show notification
        let _ = std::process::Command::new("osascript")
            .arg("-e")
            .arg("display notification \"Recording complete! Check the response box.\" with title \"GIA Audio Recording\"")
            .output();
    }

    #[cfg(not(target_os = "macos"))]
    {
        // On Windows and Linux, use notify-rust
        let _ = Notification::new()
            .summary("GIA Audio Recording")
            .body("Recording complete! Check the response box.")
            .icon("microphone")
            .show();
    }
}

fn main() -> eframe::Result<()> {
    let args = Args::parse();
    let version = env!("GIA_VERSION");
    let title = format!("GIA GUI - v{}", version);

    if args.spinner {
        // Spinner-only mode: small window with just a spinner, no decorations, centered
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([150.0, 150.0])
                .with_resizable(false)
                .with_decorations(false)
                .with_transparent(true)
                .with_always_on_top()
                .with_icon(load_icon()),
            centered: true,
            hardware_acceleration: eframe::HardwareAcceleration::Required,
            ..Default::default()
        };

        eframe::run_native(
            &title,
            options,
            Box::new(|cc| {
                // Set the clear color to fully transparent
                let mut visuals = egui::Visuals::dark();
                visuals.window_fill = egui::Color32::TRANSPARENT;
                visuals.panel_fill = egui::Color32::TRANSPARENT;
                visuals.extreme_bg_color = egui::Color32::TRANSPARENT;
                visuals.window_stroke = egui::Stroke::NONE;
                visuals.popup_shadow = egui::epaint::Shadow::NONE;
                cc.egui_ctx.set_visuals(visuals);

                Ok(Box::new(SpinnerApp::default()))
            }),
        )
    } else {
        // Normal GUI mode
        let options = eframe::NativeOptions {
            viewport: egui::ViewportBuilder::default()
                .with_inner_size([800.0, 600.0])
                .with_icon(load_icon()),
            ..Default::default()
        };

        eframe::run_native(
            &title,
            options,
            Box::new(|_cc| Ok(Box::new(GiaApp::default()))),
        )
    }
}

fn load_icon() -> egui::IconData {
    let icon_bytes = include_bytes!("../../icons/gia.png");
    let image = image::load_from_memory(icon_bytes)
        .expect("Failed to load icon")
        .to_rgba8();
    let (width, height) = image.dimensions();

    egui::IconData {
        rgba: image.into_raw(),
        width,
        height,
    }
}

struct SpinnerApp {
    animation_time: f64,
}

impl Default for SpinnerApp {
    fn default() -> Self {
        Self {
            animation_time: 0.0,
        }
    }
}

impl eframe::App for SpinnerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update animation time
        self.animation_time += ctx.input(|i| i.stable_dt as f64);
        ctx.request_repaint();

        // Set background to be completely transparent
        ctx.set_pixels_per_point(1.0);

        // Set the visuals to have a transparent background
        let mut visuals = ctx.style().visuals.clone();
        visuals.window_fill = egui::Color32::TRANSPARENT;
        visuals.panel_fill = egui::Color32::TRANSPARENT;
        visuals.extreme_bg_color = egui::Color32::TRANSPARENT;
        ctx.set_visuals(visuals);

        // Make background fully transparent
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(egui::Color32::TRANSPARENT)
                    .inner_margin(egui::Margin::ZERO)
                    .outer_margin(egui::Margin::ZERO),
            )
            .show(ctx, |ui| {
                // Draw animated spinner directly on transparent background
                let num_dots = 8;
                let radius = 20.0;
                let dot_radius = 4.0;
                let center = egui::pos2(75.0, 75.0); // Center of 150x150 window

                for i in 0..num_dots {
                    let angle = (self.animation_time * 2.0) as f32
                        + (i as f32 * std::f32::consts::TAU / num_dots as f32);
                    let x = center.x + angle.cos() * radius;
                    let y = center.y + angle.sin() * radius;

                    let opacity =
                        ((self.animation_time * 3.0 + i as f64 * 0.5).sin() * 0.5 + 0.5) as f32;
                    let color = egui::Color32::from_rgba_unmultiplied(
                        100,
                        150,
                        255,
                        (opacity * 255.0) as u8,
                    );

                    ui.painter()
                        .circle_filled(egui::pos2(x, y), dot_radius, color);
                }
            });
    }
}

struct GiaApp {
    prompt: String,
    options: String,
    use_clipboard: bool,
    browser_output: bool,
    resume: bool,
    response: String,
    first_frame: bool,
    model: String,
    task: String,
    role: String,
    tasks: Vec<String>,
    roles: Vec<String>,
    ollama_models: Arc<Mutex<Vec<String>>>,
    is_executing: Arc<Mutex<bool>>,
    animation_time: f64,
    pending_response: Arc<Mutex<Option<String>>>,
    tts_enabled: bool,
    tts_language: String,
    logo_texture: Option<egui::TextureHandle>,
}

impl Default for GiaApp {
    fn default() -> Self {
        let tasks = load_md_files("tasks");
        let roles = load_md_files("roles");

        // Fetch Ollama models in background thread
        let ollama_models = Arc::new(Mutex::new(Vec::new()));
        let ollama_models_clone = Arc::clone(&ollama_models);

        thread::spawn(move || {
            let models = fetch_ollama_models();
            *ollama_models_clone.lock().unwrap() = models;
        });

        Self {
            prompt: String::new(),
            options: String::new(),
            use_clipboard: false,
            browser_output: false,
            resume: false,
            response: String::new(),
            first_frame: true,
            model: "gemini-2.5-flash-lite".to_string(),
            task: String::new(),
            role: String::new(),
            tasks,
            roles,
            ollama_models,
            is_executing: Arc::new(Mutex::new(false)),
            animation_time: 0.0,
            pending_response: Arc::new(Mutex::new(None)),
            tts_enabled: false,
            tts_language: "de-DE".to_string(),
            logo_texture: None,
        }
    }
}

fn load_md_files(subdir: &str) -> Vec<String> {
    let mut files = Vec::new();

    if let Some(home_dir) = dirs::home_dir() {
        let path = home_dir.join(".gia").join(subdir);

        if let Ok(entries) = fs::read_dir(path) {
            for entry in entries.flatten() {
                if let Ok(file_type) = entry.file_type()
                    && file_type.is_file()
                    && let Some(file_name) = entry.file_name().to_str()
                    && file_name.ends_with(".md")
                {
                    let name = file_name.trim_end_matches(".md").to_string();
                    files.push(name);
                }
            }
        }
    }

    files.sort();
    files
}

impl eframe::App for GiaApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Cache mutex values at the start
        let is_executing = *self.is_executing.lock().unwrap();

        // Check for pending response
        if let Ok(mut pending) = self.pending_response.lock()
            && let Some(response) = pending.take()
        {
            self.response = response;
        }

        // Request repaint for animation (use cached value)
        if is_executing {
            self.animation_time += ctx.input(|i| i.stable_dt as f64);
            ctx.request_repaint();
        }

        // Handle keyboard shortcuts
        if ctx.input(|i| i.key_pressed(egui::Key::Enter) && i.modifiers.ctrl) {
            self.send_prompt();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::R) && i.modifiers.ctrl) {
            self.send_prompt_with_audio();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::L) && i.modifiers.ctrl) {
            self.clear_form();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::C) && i.modifiers.ctrl && i.modifiers.shift) {
            self.copy_response();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::O) && i.modifiers.ctrl) {
            self.show_conversation();
        }
        if ctx.input(|i| i.key_pressed(egui::Key::F1)) {
            self.show_help();
        }
        // Checkbox shortcuts
        if ctx.input(|i| i.key_pressed(egui::Key::Num1) && i.modifiers.ctrl) {
            self.resume = !self.resume;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Num2) && i.modifiers.ctrl) {
            self.use_clipboard = !self.use_clipboard;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Num3) && i.modifiers.ctrl) {
            self.browser_output = !self.browser_output;
        }
        if ctx.input(|i| i.key_pressed(egui::Key::Num4) && i.modifiers.ctrl) {
            self.tts_enabled = !self.tts_enabled;
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.vertical(|ui| {
                // Prompt input
                ui.vertical(|ui| {
                    ui.label("Prompt:");
                    let prompt_response = egui::ScrollArea::vertical()
                        .max_height(60.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.prompt)
                                    .desired_width(f32::INFINITY)
                                    .desired_rows(3),
                            )
                        })
                        .inner;

                    // Request focus on first frame
                    if self.first_frame {
                        prompt_response.request_focus();
                        self.first_frame = false;
                    }
                });

                // Handle drag and drop
                if !ctx.input(|i| i.raw.dropped_files.is_empty()) {
                    let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
                    for file in dropped_files {
                        if let Some(path) = file.path
                            && let Some(path_str) = path.to_str()
                        {
                            let option_line = format!("-f{}", path_str);

                            if !self.options.is_empty() && !self.options.ends_with('\n') {
                                self.options.push('\n');
                            }
                            self.options.push_str(&option_line);
                        }
                    }
                }

                ui.add_space(10.0);

                // Options group and custom options side by side
                ui.horizontal(|ui| {
                    // Checkboxes
                    ui.group(|ui| {
                        ui.vertical(|ui| {
                            ui.checkbox(
                                &mut self.resume,
                                "📥 Resume last conversation (-R) [Ctrl+1]",
                            );
                            ui.checkbox(
                                &mut self.use_clipboard,
                                "📥 Use clipboard input (-c) [Ctrl+2]",
                            );
                            ui.add_space(3.0);
                            ui.checkbox(
                                &mut self.browser_output,
                                "📤 Browser output (--browser-output) [Ctrl+3]",
                            );
                            ui.checkbox(
                                &mut self.tts_enabled,
                                "📤 Text-to-Speech (--tts) [Ctrl+4]",
                            );
                        });
                    });

                    // GIA logo (load once and cache)
                    if self.logo_texture.is_none() {
                        let logo_bytes = include_bytes!("../../icons/gia.png");
                        if let Ok(image) = image::load_from_memory(logo_bytes) {
                            let image =
                                image.resize_exact(80, 80, image::imageops::FilterType::Lanczos3);
                            let rgba_image = image.to_rgba8();
                            let pixels = rgba_image.as_flat_samples();
                            let color_image = egui::ColorImage::from_rgba_unmultiplied(
                                [80, 80],
                                pixels.as_slice(),
                            );
                            self.logo_texture = Some(ui.ctx().load_texture(
                                "gia_logo",
                                color_image,
                                egui::TextureOptions::LINEAR,
                            ));
                        }
                    }

                    if let Some(ref texture) = self.logo_texture {
                        ui.add_space(10.0);
                        ui.image(texture);
                        ui.add_space(10.0);
                    }

                    // Custom options input
                    ui.vertical(|ui| {
                        ui.label("GIA Command Line Options: (Drop files or folders here)");
                        let options_lines = self.options.lines().count().clamp(1, 10);
                        egui::ScrollArea::vertical()
                            .max_height(200.0)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.options)
                                        .desired_width(f32::INFINITY)
                                        .desired_rows(options_lines),
                                )
                            });
                        ui.horizontal(|ui| {
                            // Left column: Model and TTS Language
                            ui.vertical(|ui| {
                                let model_width = ui
                                    .horizontal(|ui| {
                                        ui.label("💡");
                                        egui::ComboBox::from_id_salt("model_selector")
                                            .selected_text(&self.model)
                                            .show_ui(ui, |ui| {
                                                ui.label("Gemini Models:");
                                                ui.selectable_value(
                                                    &mut self.model,
                                                    "gemini-2.5-pro".to_string(),
                                                    "Gemini 2.5 Pro",
                                                );
                                                ui.selectable_value(
                                                    &mut self.model,
                                                    "gemini-2.5-flash".to_string(),
                                                    "Gemini 2.5 Flash",
                                                );
                                                ui.selectable_value(
                                                    &mut self.model,
                                                    "gemini-2.5-flash-lite".to_string(),
                                                    "Gemini 2.5 Flash-Lite",
                                                );
                                                ui.selectable_value(
                                                    &mut self.model,
                                                    "gemini-2.0-flash".to_string(),
                                                    "Gemini 2.0 Flash",
                                                );
                                                ui.selectable_value(
                                                    &mut self.model,
                                                    "gemini-2.0-flash-lite".to_string(),
                                                    "Gemini 2.0 Flash-Lite",
                                                );

                                                // Add Ollama models if available
                                                let ollama_models = {
                                                    let models = self.ollama_models.lock().unwrap();
                                                    models.clone()
                                                };
                                                if !ollama_models.is_empty() {
                                                    ui.separator();
                                                    ui.label("Ollama Models:");
                                                    for model in ollama_models.iter() {
                                                        let display_name = model
                                                            .strip_prefix("ollama::")
                                                            .unwrap_or(model);
                                                        ui.selectable_value(
                                                            &mut self.model,
                                                            model.clone(),
                                                            format!("Ollama {}", display_name),
                                                        );
                                                    }
                                                }
                                            })
                                            .response
                                            .rect
                                            .width()
                                    })
                                    .inner;

                                ui.horizontal(|ui| {
                                    ui.label("💬");
                                    egui::ComboBox::from_id_salt("tts_language_selector")
                                        .selected_text(&self.tts_language)
                                        .width(model_width)
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut self.tts_language,
                                                "de-DE".to_string(),
                                                "de-DE",
                                            );
                                            ui.selectable_value(
                                                &mut self.tts_language,
                                                "en-US".to_string(),
                                                "en-US",
                                            );
                                        });
                                });
                            });

                            // Right column: Task and Role
                            ui.vertical(|ui| {
                                ui.horizontal(|ui| {
                                    ui.label("📋");
                                    egui::ComboBox::from_id_salt("task_selector")
                                        .selected_text(if self.task.is_empty() {
                                            "Select Task"
                                        } else {
                                            &self.task
                                        })
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut self.task,
                                                String::new(),
                                                "None",
                                            );
                                            for task in &self.tasks {
                                                ui.selectable_value(
                                                    &mut self.task,
                                                    task.clone(),
                                                    task,
                                                );
                                            }
                                        });
                                });

                                ui.horizontal(|ui| {
                                    ui.label("👤");
                                    egui::ComboBox::from_id_salt("role_selector")
                                        .selected_text(if self.role.is_empty() {
                                            "Select Role"
                                        } else {
                                            &self.role
                                        })
                                        .show_ui(ui, |ui| {
                                            ui.selectable_value(
                                                &mut self.role,
                                                String::new(),
                                                "None",
                                            );
                                            for role in &self.roles {
                                                ui.selectable_value(
                                                    &mut self.role,
                                                    role.clone(),
                                                    role,
                                                );
                                            }
                                        });
                                });
                            });
                        });
                    });
                });

                ui.add_space(10.0);

                // Buttons
                ui.horizontal(|ui| {
                    if ui.button("📨 Send (Ctrl+Enter)").clicked() {
                        self.send_prompt();
                    }
                    if ui.button("🔴 Record (Ctrl+R)").clicked() {
                        self.send_prompt_with_audio();
                    }
                    if ui.button("❌ Clear (Ctrl+L)").clicked() {
                        self.clear_form();
                    }
                    if ui.button("📋 Copy (Ctrl+Shift+C)").clicked() {
                        self.copy_response();
                    }
                    if ui.button("💬 Conversation (Ctrl+O)").clicked() {
                        self.show_conversation();
                    }
                    if ui.button("❓ Help (F1)").clicked() {
                        self.show_help();
                    }
                });

                ui.add_space(5.0);

                // Animation during execution (use cached value)
                if is_executing {
                    ui.horizontal(|ui| {
                        ui.label("Executing GIA");

                        // Animated spinner with rotating dots
                        let num_dots = 8;
                        let radius = 8.0;
                        let dot_radius = 2.5;
                        let center = ui.cursor().min + egui::vec2(30.0, 10.0);

                        for i in 0..num_dots {
                            let angle = (self.animation_time * 2.0) as f32
                                + (i as f32 * std::f32::consts::TAU / num_dots as f32);
                            let x = center.x + angle.cos() * radius;
                            let y = center.y + angle.sin() * radius;

                            let opacity = ((self.animation_time * 3.0 + i as f64 * 0.5).sin() * 0.5
                                + 0.5) as f32;
                            let color = egui::Color32::from_rgba_unmultiplied(
                                100,
                                150,
                                255,
                                (opacity * 255.0) as u8,
                            );

                            ui.painter()
                                .circle_filled(egui::pos2(x, y), dot_radius, color);
                        }
                    });
                    ui.add_space(5.0);
                }

                // Response box - use remaining space
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.add_sized(
                        ui.available_size(),
                        egui::TextEdit::multiline(&mut self.response)
                            .font(egui::TextStyle::Monospace),
                    );
                });
            });
        });
    }
}

impl GiaApp {
    fn send_prompt(&mut self) {
        self.execute_gia(false);
    }

    fn send_prompt_with_audio(&mut self) {
        self.execute_gia(true);
    }

    fn execute_gia(&mut self, with_audio: bool) {
        let mut args = vec![];

        if with_audio {
            args.push("--record-audio".to_string());
        }
        if self.use_clipboard {
            args.push("-c".to_string());
        }
        if self.browser_output {
            args.push("--browser-output".to_string());
        }
        if self.resume {
            args.push("-R".to_string());
        }

        // Add model option
        args.push("--model".to_string());
        args.push(self.model.clone());

        // Add task option if selected
        if !self.task.is_empty() {
            args.push("-t".to_string());
            args.push(self.task.clone());
        }

        // Add role option if selected
        if !self.role.is_empty() {
            args.push("--role".to_string());
            args.push(self.role.clone());
        }

        // Add TTS option if enabled
        if self.tts_enabled {
            args.push(format!("--tts={}", self.tts_language));
        }

        // Add custom options from options field
        for line in self.options.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                args.push(trimmed.to_string());
            }
        }

        if !self.prompt.is_empty() {
            args.push(self.prompt.clone());
        }

        // Clear task and role selections, uncheck clipboard, and enable resume after sending
        self.task.clear();
        self.role.clear();
        self.use_clipboard = false;
        self.resume = true;

        // Start animation
        *self.is_executing.lock().unwrap() = true;
        self.animation_time = 0.0;

        let is_executing = Arc::clone(&self.is_executing);
        let pending_response = Arc::clone(&self.pending_response);

        // Check if clipboard output mode is enabled
        let has_clipboard_output = args.iter().any(|arg| arg == "-o" || arg == "--output");

        thread::spawn(move || {
            let result = match Command::new("gia").args(&args).output() {
                Ok(output) => {
                    let mut response = String::from_utf8_lossy(&output.stdout).to_string();
                    if !output.stderr.is_empty() {
                        response.push_str("\n\nErrors/Logging:\n");
                        response.push_str(&String::from_utf8_lossy(&output.stderr));
                    }
                    response
                }
                Err(e) => format!("Error executing gia: {}", e),
            };

            // Show notification only if audio recording was used AND output is to clipboard
            if with_audio && has_clipboard_output {
                show_completion_notification();
            }

            *pending_response.lock().unwrap() = Some(result);
            *is_executing.lock().unwrap() = false;
        });
    }

    fn clear_form(&mut self) {
        self.prompt.clear();
        self.options.clear();
        self.response.clear();
        self.use_clipboard = false;
        self.browser_output = false;
        self.resume = false;
    }

    fn copy_response(&mut self) {
        if let Ok(mut clipboard) = Clipboard::new() {
            let _ = clipboard.set_text(&self.response);
        }
    }

    fn show_conversation(&mut self) {
        let mut args = vec!["--show-conversation".to_string()];

        if self.tts_enabled {
            args.push(format!("--tts={}", self.tts_language));
        }

        let _ = Command::new("gia").args(args).spawn();
    }

    fn show_help(&mut self) {
        match Command::new("gia").arg("--help").output() {
            Ok(output) => {
                self.response = String::from_utf8_lossy(&output.stdout).to_string();
                if !output.stderr.is_empty() {
                    self.response.push_str("\n\nErrors/Logging:\n");
                    self.response
                        .push_str(&String::from_utf8_lossy(&output.stderr));
                }
            }
            Err(e) => {
                self.response = format!("Error executing gia: {}", e);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;

    #[test]
    #[serial]
    fn test_fetch_ollama_models_unreachable_host() {
        unsafe { std::env::set_var("OLLAMA_API_BASE", "http://127.0.0.1:9999") };
        let result = fetch_ollama_models();
        assert!(result.is_empty());
        unsafe { std::env::remove_var("OLLAMA_API_BASE") };
    }
}
