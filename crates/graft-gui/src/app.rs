use eframe::egui;
use graft_core::patch::{apply::apply_entry, MANIFEST_FILENAME};
use graft_core::utils::manifest::{Manifest, ManifestEntry};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

/// Application state machine states
#[derive(Debug, Clone)]
pub enum AppState {
    /// Initial state showing patch info and folder selection button
    Welcome,
    /// User has selected a folder, ready to apply
    FolderSelected { path: PathBuf },
    /// Patch is being applied
    Applying {
        path: PathBuf,
        current_file: String,
        progress: f32,
        total: usize,
        completed: usize,
    },
    /// Patch applied successfully
    Success { path: PathBuf, files_patched: usize },
    /// An error occurred
    Error {
        message: String,
        details: Option<String>,
        show_details: bool,
    },
}

/// Progress update message from worker thread
#[derive(Debug)]
pub enum ProgressMessage {
    /// Starting to process a file
    Processing { file: String, index: usize, total: usize },
    /// Patch completed successfully
    Done { files_patched: usize },
    /// An error occurred
    Error { message: String, details: Option<String> },
}

/// Patch information loaded from manifest
#[derive(Debug, Clone)]
pub struct PatchInfo {
    pub version: u32,
    pub entry_count: usize,
    pub patches: usize,
    pub additions: usize,
    pub deletions: usize,
}

impl PatchInfo {
    pub fn from_manifest(manifest: &Manifest) -> Self {
        let mut patches = 0;
        let mut additions = 0;
        let mut deletions = 0;
        for entry in &manifest.entries {
            match entry {
                ManifestEntry::Patch { .. } => patches += 1,
                ManifestEntry::Add { .. } => additions += 1,
                ManifestEntry::Delete { .. } => deletions += 1,
            }
        }
        PatchInfo {
            version: manifest.version,
            entry_count: manifest.entries.len(),
            patches,
            additions,
            deletions,
        }
    }

    pub fn mock() -> Self {
        PatchInfo {
            version: 1,
            entry_count: 42,
            patches: 35,
            additions: 5,
            deletions: 2,
        }
    }
}

/// Main application struct
pub struct GraftApp {
    state: AppState,
    patch_info: PatchInfo,
    /// Path to extracted patch directory (or None for demo mode)
    patch_dir: Option<PathBuf>,
    /// Manifest entries (or None for demo mode)
    manifest: Option<Manifest>,
    /// Channel for receiving progress updates from worker thread
    progress_rx: Option<mpsc::Receiver<ProgressMessage>>,
    /// Demo mode flag
    demo_mode: bool,
    /// Text input for manual path entry
    path_input: String,
}

impl GraftApp {
    /// Create a new app in demo mode with mock data
    pub fn demo() -> Self {
        GraftApp {
            state: AppState::Welcome,
            patch_info: PatchInfo::mock(),
            patch_dir: None,
            manifest: None,
            progress_rx: None,
            demo_mode: true,
            path_input: String::new(),
        }
    }

    /// Create a new app with real patch data
    pub fn new(patch_dir: PathBuf, manifest: Manifest) -> Self {
        let patch_info = PatchInfo::from_manifest(&manifest);
        GraftApp {
            state: AppState::Welcome,
            patch_info,
            patch_dir: Some(patch_dir),
            manifest: Some(manifest),
            progress_rx: None,
            demo_mode: false,
            path_input: String::new(),
        }
    }

    fn select_folder(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.state = AppState::FolderSelected { path };
        }
    }

    fn start_apply(&mut self, target_path: PathBuf) {
        if self.demo_mode {
            // Demo mode: simulate applying
            self.state = AppState::Applying {
                path: target_path,
                current_file: "demo_file.bin".to_string(),
                progress: 0.0,
                total: self.patch_info.entry_count,
                completed: 0,
            };
            return;
        }

        let (tx, rx) = mpsc::channel();
        self.progress_rx = Some(rx);

        let patch_dir = self.patch_dir.clone().unwrap();
        let manifest = self.manifest.clone().unwrap();
        let total = manifest.entries.len();

        self.state = AppState::Applying {
            path: target_path.clone(),
            current_file: String::new(),
            progress: 0.0,
            total,
            completed: 0,
        };

        // Spawn worker thread
        thread::spawn(move || {
            for (i, entry) in manifest.entries.iter().enumerate() {
                let file = entry.file().to_string();
                let _ = tx.send(ProgressMessage::Processing {
                    file: file.clone(),
                    index: i,
                    total,
                });

                if let Err(e) = apply_entry(entry, &target_path, &patch_dir) {
                    let _ = tx.send(ProgressMessage::Error {
                        message: format!("Failed to apply patch to '{}'", file),
                        details: Some(e.to_string()),
                    });
                    return;
                }
            }

            let _ = tx.send(ProgressMessage::Done {
                files_patched: manifest.entries.len(),
            });
        });
    }

    fn process_progress_messages(&mut self) {
        // Collect messages first to avoid borrow issues
        let messages: Vec<_> = self
            .progress_rx
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();

        let mut should_clear_rx = false;

        for msg in messages {
            match msg {
                ProgressMessage::Processing { file, index, total } => {
                    if let AppState::Applying { path, .. } = &self.state {
                        self.state = AppState::Applying {
                            path: path.clone(),
                            current_file: file,
                            progress: index as f32 / total as f32,
                            total,
                            completed: index,
                        };
                    }
                }
                ProgressMessage::Done { files_patched } => {
                    if let AppState::Applying { path, .. } = &self.state {
                        self.state = AppState::Success {
                            path: path.clone(),
                            files_patched,
                        };
                    }
                    should_clear_rx = true;
                }
                ProgressMessage::Error { message, details } => {
                    self.state = AppState::Error {
                        message,
                        details,
                        show_details: false,
                    };
                    should_clear_rx = true;
                }
            }
        }

        if should_clear_rx {
            self.progress_rx = None;
        }
    }

    fn render_welcome(&mut self, ui: &mut egui::Ui) {
        ui.heading("Patch Ready to Apply");
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.label(format!("Version: {}", self.patch_info.version));
            ui.label(format!("Total operations: {}", self.patch_info.entry_count));
            ui.add_space(8.0);
            ui.horizontal(|ui| {
                ui.label(format!("{} patches", self.patch_info.patches));
                ui.separator();
                ui.label(format!("{} additions", self.patch_info.additions));
                ui.separator();
                ui.label(format!("{} deletions", self.patch_info.deletions));
            });
        });

        ui.add_space(24.0);

        ui.horizontal(|ui| {
            if ui.button("Select Folder...").clicked() {
                self.select_folder();
            }
        });

        ui.add_space(8.0);
        ui.label("Or enter path manually:");
        ui.horizontal(|ui| {
            ui.add(egui::TextEdit::singleline(&mut self.path_input).hint_text("/path/to/folder").desired_width(250.0));
            let path = PathBuf::from(&self.path_input);
            let valid = !self.path_input.is_empty() && path.is_absolute();
            if ui.add_enabled(valid, egui::Button::new("Use Path")).clicked() {
                self.state = AppState::FolderSelected { path };
            }
        });

        if self.demo_mode {
            ui.add_space(8.0);
            ui.label(egui::RichText::new("(Demo Mode)").color(egui::Color32::GRAY).italics());
        }
    }

    fn render_folder_selected(&mut self, ui: &mut egui::Ui, path: PathBuf) {
        ui.heading("Ready to Apply");
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.label("Target folder:");
            ui.label(egui::RichText::new(path.display().to_string()).monospace());
        });

        ui.add_space(16.0);
        ui.label(format!(
            "This will apply {} operations to the selected folder.",
            self.patch_info.entry_count
        ));

        ui.add_space(24.0);

        ui.horizontal(|ui| {
            if ui.button("Apply Patch").clicked() {
                self.start_apply(path.clone());
            }
            if ui.button("Change Folder...").clicked() {
                self.select_folder();
            }
        });
    }

    fn render_applying(&mut self, ui: &mut egui::Ui, current_file: String, progress: f32, total: usize, completed: usize) {
        ui.heading("Applying Patch...");
        ui.add_space(16.0);

        ui.add(egui::ProgressBar::new(progress).show_percentage());
        ui.add_space(8.0);

        ui.label(format!("Processing: {}", current_file));
        ui.label(format!("{} / {} operations completed", completed, total));

        // Demo mode: simulate progress
        if self.demo_mode {
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if ui.button("Simulate Progress").clicked() {
                    if let AppState::Applying { path, total, completed, .. } = &self.state {
                        let new_completed = (completed + 1).min(*total);
                        let new_progress = new_completed as f32 / *total as f32;
                        if new_completed >= *total {
                            self.state = AppState::Success {
                                path: path.clone(),
                                files_patched: *total,
                            };
                        } else {
                            self.state = AppState::Applying {
                                path: path.clone(),
                                current_file: format!("file_{}.bin", new_completed),
                                progress: new_progress,
                                total: *total,
                                completed: new_completed,
                            };
                        }
                    }
                }
                if ui.button("Simulate Error").clicked() {
                    self.state = AppState::Error {
                        message: "Failed to apply patch".to_string(),
                        details: Some("Demo error: This is a simulated error for testing the error state display.".to_string()),
                        show_details: false,
                    };
                }
            });
        }
    }

    fn render_success(&self, ctx: &egui::Context, ui: &mut egui::Ui, path: &PathBuf, files_patched: usize) {
        ui.vertical_centered(|ui| {
            ui.add_space(24.0);

            // Green circle with white checkmark
            let (rect, _) = ui.allocate_exact_size(egui::vec2(80.0, 80.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 40.0, egui::Color32::from_rgb(34, 197, 94));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "\u{2713}",
                egui::FontId::proportional(48.0),
                egui::Color32::WHITE,
            );

            ui.add_space(16.0);
            ui.heading("Patch Applied Successfully!");
            ui.add_space(16.0);
            ui.label(format!("{} operations completed", files_patched));
            ui.add_space(8.0);
            ui.label(egui::RichText::new(path.display().to_string()).monospace().small());
            ui.add_space(24.0);

            if ui.button("Quit").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }

    fn render_error(&mut self, ctx: &egui::Context, ui: &mut egui::Ui, message: String, details: Option<String>, show_details: bool) {
        ui.vertical_centered(|ui| {
            ui.add_space(24.0);

            // Red circle with white X
            let (rect, _) = ui.allocate_exact_size(egui::vec2(80.0, 80.0), egui::Sense::hover());
            ui.painter().circle_filled(rect.center(), 40.0, egui::Color32::from_rgb(239, 68, 68));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "\u{2717}",
                egui::FontId::proportional(48.0),
                egui::Color32::WHITE,
            );

            ui.add_space(16.0);
            ui.heading("Error");
        });

        ui.add_space(16.0);
        ui.label(&message);

        if let Some(ref detail_text) = details {
            ui.add_space(8.0);
            let button_text = if show_details { "Hide Details" } else { "Show Details" };
            if ui.button(button_text).clicked() {
                self.state = AppState::Error {
                    message: message.clone(),
                    details: details.clone(),
                    show_details: !show_details,
                };
            }

            if show_details {
                ui.add_space(8.0);
                egui::ScrollArea::vertical().max_height(100.0).show(ui, |ui| {
                    ui.label(egui::RichText::new(detail_text).monospace().small());
                });
            }
        }

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("Try Again").clicked() {
                self.state = AppState::Welcome;
            }
            if ui.button("Quit").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}

impl eframe::App for GraftApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process any pending progress messages
        self.process_progress_messages();

        // Request repaint if we're applying (to get progress updates)
        if matches!(self.state, AppState::Applying { .. }) {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(16.0);

            // Clone state to avoid borrow issues
            let state = self.state.clone();
            match state {
                AppState::Welcome => self.render_welcome(ui),
                AppState::FolderSelected { path } => self.render_folder_selected(ui, path),
                AppState::Applying { current_file, progress, total, completed, .. } => {
                    self.render_applying(ui, current_file, progress, total, completed)
                }
                AppState::Success { path, files_patched } => self.render_success(ctx, ui, &path, files_patched),
                AppState::Error { message, details, show_details } => {
                    self.render_error(ctx, ui, message, details, show_details)
                }
            }
        });
    }
}

/// Run the GUI application
pub fn run(patch_data: Option<&[u8]>) -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 380.0])
            .with_min_inner_size([350.0, 340.0]),
        ..Default::default()
    };

    let app: GraftApp = if let Some(data) = patch_data {
        // Extract patch data and create real app
        match extract_and_load_patch(data) {
            Ok((patch_dir, manifest)) => GraftApp::new(patch_dir, manifest),
            Err(e) => {
                eprintln!("Failed to load embedded patch: {}", e);
                return Err(eframe::Error::AppCreation(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e,
                ))));
            }
        }
    } else {
        GraftApp::demo()
    };

    eframe::run_native("Graft Patcher", options, Box::new(|cc| {
        // Use light theme
        cc.egui_ctx.set_visuals(egui::Visuals::light());
        Ok(Box::new(app))
    }))
}

/// Extract patch data from compressed tar archive and load manifest
fn extract_and_load_patch(data: &[u8]) -> Result<(PathBuf, Manifest), String> {
    use flate2::read::GzDecoder;
    use tar::Archive;

    // Create temp directory for extracted patch
    let temp_dir = tempfile::tempdir()
        .map_err(|e| format!("Failed to create temp directory: {}", e))?;

    // Decompress and extract
    let decoder = GzDecoder::new(data);
    let mut archive = Archive::new(decoder);
    archive
        .unpack(temp_dir.path())
        .map_err(|e| format!("Failed to extract patch archive: {}", e))?;

    // Load manifest
    let manifest_path = temp_dir.path().join(MANIFEST_FILENAME);
    let manifest = Manifest::load(&manifest_path)
        .map_err(|e| format!("Failed to load manifest: {}", e))?;

    // Keep temp_dir alive by converting to path (will be cleaned up on process exit)
    let patch_dir = temp_dir.keep();

    Ok((patch_dir, manifest))
}
