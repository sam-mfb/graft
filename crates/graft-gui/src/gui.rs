use crate::runner::{PatchRunner, Phase, ProgressAction, ProgressEvent, RollbackEvent};
use crate::validator::{PatchInfo, PatchValidationError, PatchValidator};
use eframe::egui;
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

fn format_action(action: ProgressAction) -> &'static str {
    match action {
        ProgressAction::Validating => "Validating",
        ProgressAction::CheckingNotExists => "Checking",
        ProgressAction::BackingUp => "Backing up",
        ProgressAction::Skipping => "Skipping",
        ProgressAction::Patching => "Patching",
        ProgressAction::Adding => "Adding",
        ProgressAction::Deleting => "Deleting",
        ProgressAction::Restoring => "Restoring",
        ProgressAction::Removing => "Removing",
    }
}

/// Application state machine states
#[derive(Debug, Clone)]
pub enum AppState {
    /// Initial state showing patch info and folder selection button
    Welcome,
    /// Validating selected folder before showing ready state
    ValidatingFolder { path: PathBuf },
    /// Folder validation failed - cannot patch this folder
    FolderInvalid { path: PathBuf, reason: String },
    /// Folder already has backup - appears to be patched already
    AlreadyPatched { path: PathBuf, modified: bool },
    /// User has selected a folder, ready to apply
    FolderSelected { path: PathBuf },
    /// Patch is being applied
    Applying {
        path: PathBuf,
        progress: f32,
        current_phase: Option<Phase>,
        completed_phases: usize,
        phase_total: usize,
        log: Vec<String>,
    },
    /// Patch applied successfully
    Success {
        path: PathBuf,
        files_patched: usize,
        log: Vec<String>,
    },
    /// Rollback in progress
    RollingBack {
        path: PathBuf,
        log: Vec<String>,
    },
    /// Rollback needs confirmation - target files have changed
    RollbackWarning { path: PathBuf, reason: String },
    /// Rollback completed successfully
    RollbackComplete { path: PathBuf, log: Vec<String> },
    /// An error occurred
    Error {
        message: String,
        details: Option<String>,
        show_details: bool,
        log: Vec<String>,
    },
}

/// Result of folder validation
#[derive(Debug, Clone)]
pub enum ValidationResult {
    /// Folder is valid for patching
    Valid,
    /// Folder cannot be patched
    Invalid { reason: String },
    /// Folder appears to already be patched
    AlreadyPatched { modified: bool },
}

/// Application mode
pub enum Mode {
    /// Demo mode with mock data for UI development
    Demo,
    /// Real mode with embedded patch data
    Embedded {
        patch_data: Vec<u8>,
        /// Channel for receiving progress updates from worker thread (Some when applying)
        progress_rx: Option<mpsc::Receiver<ProgressEvent>>,
        /// Channel for receiving validation results
        validation_rx: Option<mpsc::Receiver<ValidationResult>>,
        /// Channel for receiving rollback events
        rollback_rx: Option<mpsc::Receiver<RollbackEvent>>,
    },
}

/// Main application struct
pub struct GraftApp {
    state: AppState,
    patch_info: PatchInfo,
    mode: Mode,
    /// Text input for manual path entry
    path_input: String,
    /// Window title from manifest
    title: String,
}

impl GraftApp {
    /// Create a new app in demo mode with mock data
    pub fn demo() -> Self {
        let patch_info = PatchInfo::mock();
        let title = patch_info
            .title
            .clone()
            .unwrap_or_else(|| "Graft Patcher".to_string());
        GraftApp {
            state: AppState::Welcome,
            patch_info,
            mode: Mode::Demo,
            path_input: String::new(),
            title,
        }
    }

    /// Create a new app with patch data
    ///
    /// Validates the patch to get PatchInfo for display, then stores
    /// the raw data for the worker thread to use when applying.
    pub fn new(patch_data: Vec<u8>) -> Result<Self, PatchValidationError> {
        let patch_info = PatchValidator::validate(&patch_data)?;
        let title = patch_info
            .title
            .clone()
            .unwrap_or_else(|| "Graft Patcher".to_string());

        Ok(GraftApp {
            state: AppState::Welcome,
            patch_info,
            mode: Mode::Embedded {
                patch_data,
                progress_rx: None,
                validation_rx: None,
                rollback_rx: None,
            },
            path_input: String::new(),
            title,
        })
    }

    fn select_folder(&mut self) {
        if let Some(path) = rfd::FileDialog::new().pick_folder() {
            self.start_validation(path);
        }
    }

    fn start_validation(&mut self, path: PathBuf) {
        let patch_data = match &mut self.mode {
            Mode::Demo => {
                // Demo mode: skip validation, go directly to FolderSelected
                self.state = AppState::FolderSelected { path };
                return;
            }
            Mode::Embedded { patch_data, validation_rx, .. } => {
                let data = patch_data.clone();
                let (tx, rx) = mpsc::channel();
                *validation_rx = Some(rx);
                (data, tx)
            }
        };

        self.state = AppState::ValidatingFolder { path: path.clone() };

        let (patch_data, tx) = patch_data;

        // Worker thread validates the folder
        thread::spawn(move || {
            let runner = match PatchRunner::new(&patch_data) {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(ValidationResult::Invalid {
                        reason: e.to_string(),
                    });
                    return;
                }
            };

            // Check if backup exists (already patched scenario)
            if PatchRunner::has_backup(&path) {
                // Check if files are in patched state
                let modified = !runner.is_patched(&path);
                let _ = tx.send(ValidationResult::AlreadyPatched { modified });
                return;
            }

            // Validate folder can be patched
            match runner.validate_target(&path) {
                Ok(()) => {
                    let _ = tx.send(ValidationResult::Valid);
                }
                Err(e) => {
                    let _ = tx.send(ValidationResult::Invalid {
                        reason: e.to_string(),
                    });
                }
            }
        });
    }

    fn start_apply(&mut self, target_path: PathBuf) {
        let patch_data = match &mut self.mode {
            Mode::Demo => {
                // Demo mode: simulate applying
                self.state = AppState::Applying {
                    path: target_path,
                    progress: 0.0,
                    current_phase: Some(Phase::Applying),
                    completed_phases: 0,
                    phase_total: self.patch_info.entry_count,
                    log: vec!["[Demo] Starting patch application...".to_string()],
                };
                return;
            }
            Mode::Embedded { patch_data, progress_rx, .. } => {
                let data = patch_data.clone();
                let (tx, rx) = mpsc::channel();
                *progress_rx = Some(rx);
                (data, tx)
            }
        };

        let total = self.patch_info.entry_count;

        self.state = AppState::Applying {
            path: target_path.clone(),
            progress: 0.0,
            current_phase: None,
            completed_phases: 0,
            phase_total: total,
            log: Vec::new(),
        };

        let (patch_data, tx) = patch_data;

        // Worker thread creates and owns its own runner
        thread::spawn(move || {
            let runner = match PatchRunner::new(&patch_data) {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(ProgressEvent::Error {
                        message: "Failed to create patch runner".to_string(),
                        details: Some(e.to_string()),
                    });
                    return;
                }
            };

            let _ = runner.apply(&target_path, |event| {
                let _ = tx.send(event);
            });
        });
    }

    fn process_progress_messages(&mut self) {
        let progress_rx = match &mut self.mode {
            Mode::Demo => return,
            Mode::Embedded { progress_rx, .. } => progress_rx,
        };

        // Collect messages first to avoid borrow issues
        let messages: Vec<_> = progress_rx
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();

        let mut should_clear_rx = false;

        for event in messages {
            match event {
                ProgressEvent::PhaseStarted { phase } => {
                    if let AppState::Applying {
                        log,
                        current_phase,
                        completed_phases,
                        progress,
                        phase_total,
                        ..
                    } = &mut self.state
                    {
                        // Mark previous phase as complete
                        if current_phase.is_some() {
                            *completed_phases += 1;
                        }
                        *current_phase = Some(phase);
                        log.push(format!("[{}]", phase));
                        // Update progress: each phase is 1/3 of total
                        *progress = *completed_phases as f32 / 3.0;
                        // Reset phase total (will be updated by first Operation)
                        let _ = phase_total;
                    }
                }
                ProgressEvent::Operation {
                    file,
                    index,
                    total,
                    action,
                } => {
                    if let AppState::Applying {
                        log,
                        progress,
                        completed_phases,
                        phase_total,
                        ..
                    } = &mut self.state
                    {
                        log.push(format!("  [{}/{}] {}: {}", index + 1, total, format_action(action), file));
                        *phase_total = total;
                        // Progress: completed phases + current phase progress
                        let phase_progress = (index + 1) as f32 / total.max(1) as f32;
                        *progress = (*completed_phases as f32 + phase_progress) / 3.0;
                    }
                }
                ProgressEvent::Done { files_patched } => {
                    if let AppState::Applying { path, log, .. } = &self.state {
                        self.state = AppState::Success {
                            path: path.clone(),
                            files_patched,
                            log: log.clone(),
                        };
                    }
                    should_clear_rx = true;
                }
                ProgressEvent::Error { message, details } => {
                    let log = if let AppState::Applying { log, .. } = &self.state {
                        log.clone()
                    } else {
                        Vec::new()
                    };
                    self.state = AppState::Error {
                        message,
                        details,
                        show_details: false,
                        log,
                    };
                    should_clear_rx = true;
                }
            }
        }

        if should_clear_rx {
            *progress_rx = None;
        }
    }

    fn process_validation_messages(&mut self) {
        let validation_rx = match &mut self.mode {
            Mode::Demo => return,
            Mode::Embedded { validation_rx, .. } => validation_rx,
        };

        // Get validation result if available
        let result = validation_rx
            .as_ref()
            .and_then(|rx| rx.try_recv().ok());

        if let Some(result) = result {
            if let AppState::ValidatingFolder { path } = &self.state {
                let path = path.clone();
                match result {
                    ValidationResult::Valid => {
                        self.state = AppState::FolderSelected { path };
                    }
                    ValidationResult::Invalid { reason } => {
                        self.state = AppState::FolderInvalid { path, reason };
                    }
                    ValidationResult::AlreadyPatched { modified } => {
                        self.state = AppState::AlreadyPatched { path, modified };
                    }
                }
            }
            *validation_rx = None;
        }
    }

    fn process_rollback_messages(&mut self) {
        let rollback_rx = match &mut self.mode {
            Mode::Demo => return,
            Mode::Embedded { rollback_rx, .. } => rollback_rx,
        };

        // Collect messages
        let messages: Vec<_> = rollback_rx
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();

        let mut should_clear_rx = false;

        for event in messages {
            match event {
                RollbackEvent::ValidatingTarget | RollbackEvent::ValidatingBackup => {
                    // Could add log messages here if desired
                }
                RollbackEvent::TargetModified { reason } => {
                    if let AppState::RollingBack { path, .. } = &self.state {
                        self.state = AppState::RollbackWarning {
                            path: path.clone(),
                            reason,
                        };
                    }
                    should_clear_rx = true;
                }
                RollbackEvent::Rolling { file, index, total, action } => {
                    if let AppState::RollingBack { log, .. } = &mut self.state {
                        log.push(format!("  [{}/{}] {}: {}", index + 1, total, format_action(action), file));
                    }
                }
                RollbackEvent::Done { .. } => {
                    if let AppState::RollingBack { path, log, .. } = &self.state {
                        self.state = AppState::RollbackComplete {
                            path: path.clone(),
                            log: log.clone(),
                        };
                    }
                    should_clear_rx = true;
                }
                RollbackEvent::Error { message } => {
                    if let AppState::RollingBack { log, .. } = &self.state {
                        self.state = AppState::Error {
                            message: "Rollback failed".to_string(),
                            details: Some(message),
                            show_details: false,
                            log: log.clone(),
                        };
                    }
                    should_clear_rx = true;
                }
            }
        }

        if should_clear_rx {
            *rollback_rx = None;
        }
    }

    fn start_rollback(&mut self, path: PathBuf, force: bool) {
        let patch_data = match &mut self.mode {
            Mode::Demo => {
                // Demo mode: simulate rollback complete
                self.state = AppState::RollbackComplete {
                    path,
                    log: vec!["[Demo] Rollback complete".to_string()],
                };
                return;
            }
            Mode::Embedded { patch_data, rollback_rx, .. } => {
                let data = patch_data.clone();
                let (tx, rx) = mpsc::channel();
                *rollback_rx = Some(rx);
                (data, tx)
            }
        };

        self.state = AppState::RollingBack {
            path: path.clone(),
            log: vec!["[Rolling back]".to_string()],
        };

        let (patch_data, tx) = patch_data;

        // Worker thread performs rollback
        thread::spawn(move || {
            let runner = match PatchRunner::new(&patch_data) {
                Ok(r) => r,
                Err(e) => {
                    let _ = tx.send(RollbackEvent::Error {
                        message: e.to_string(),
                    });
                    return;
                }
            };

            let _ = runner.rollback(&path, force, |event| {
                let _ = tx.send(event);
            });
        });
    }

    /// Render a scrollable log area with fixed height
    fn render_log(ui: &mut egui::Ui, log: &[String]) {
        let height = 120.0;
        egui::Frame::NONE
            .fill(ui.visuals().extreme_bg_color)
            .corner_radius(4.0)
            .inner_margin(4.0)
            .outer_margin(egui::Margin::symmetric(8, 0))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                egui::ScrollArea::vertical()
                    .max_height(height)
                    .min_scrolled_height(height)
                    .stick_to_bottom(true)
                    .show(ui, |ui| {
                        ui.set_min_height(height);
                        for line in log {
                            ui.label(egui::RichText::new(line).monospace().small());
                        }
                    });
            });
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
            ui.add(
                egui::TextEdit::singleline(&mut self.path_input)
                    .hint_text("/path/to/folder")
                    .desired_width(250.0),
            );
            let path = PathBuf::from(&self.path_input);
            let valid = !self.path_input.is_empty() && path.is_absolute();
            if ui
                .add_enabled(valid, egui::Button::new("Use Path"))
                .clicked()
            {
                self.start_validation(path);
            }
        });

        if matches!(self.mode, Mode::Demo) {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("(Demo Mode)")
                    .color(egui::Color32::GRAY)
                    .italics(),
            );
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

    fn render_applying(
        &mut self,
        ui: &mut egui::Ui,
        log: Vec<String>,
        progress: f32,
        current_phase: Option<Phase>,
    ) {
        ui.heading("Applying Patch...");
        ui.add_space(16.0);

        ui.add(egui::ProgressBar::new(progress).show_percentage());
        ui.add_space(8.0);

        if let Some(phase) = current_phase {
            ui.label(format!("Phase: {}", phase));
        }

        ui.add_space(8.0);
        Self::render_log(ui, &log);

        // Demo mode: simulate progress
        if matches!(self.mode, Mode::Demo) {
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if ui.button("Simulate Progress").clicked() {
                    if let AppState::Applying {
                        path,
                        phase_total,
                        completed_phases,
                        log,
                        current_phase,
                        ..
                    } = &self.state
                    {
                        let mut new_log = log.clone();
                        let batch_size = 10; // Simulate ~10 ops per click
                        let new_completed = (completed_phases + batch_size).min(*phase_total);
                        // Log each simulated operation in the batch
                        for i in (*completed_phases + 1)..=new_completed {
                            new_log.push(format!(
                                "  [{}/{}] Patching: file_{}.bin",
                                i, phase_total, i
                            ));
                        }
                        let new_progress = new_completed as f32 / *phase_total as f32;
                        if new_completed >= *phase_total {
                            self.state = AppState::Success {
                                path: path.clone(),
                                files_patched: *phase_total,
                                log: new_log,
                            };
                        } else {
                            self.state = AppState::Applying {
                                path: path.clone(),
                                progress: new_progress,
                                current_phase: *current_phase,
                                completed_phases: new_completed,
                                phase_total: *phase_total,
                                log: new_log,
                            };
                        }
                    }
                }
                if ui.button("Simulate Error").clicked() {
                    let log = if let AppState::Applying { log, .. } = &self.state {
                        log.clone()
                    } else {
                        Vec::new()
                    };
                    self.state = AppState::Error {
                        message: "Failed to apply patch".to_string(),
                        details: Some(
                            "Demo error: This is a simulated error for testing the error state display."
                                .to_string(),
                        ),
                        show_details: false,
                        log,
                    };
                }
            });
        }
    }

    fn render_success(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        path: PathBuf,
        files_patched: usize,
        log: &[String],
    ) {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);

            // Green circle with white checkmark
            let (rect, _) = ui.allocate_exact_size(egui::vec2(60.0, 60.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 30.0, egui::Color32::from_rgb(34, 197, 94));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "\u{2713}",
                egui::FontId::proportional(36.0),
                egui::Color32::WHITE,
            );

            ui.add_space(8.0);
            ui.heading("Patch Applied Successfully!");
            ui.add_space(4.0);
            ui.label(format!("{} operations completed", files_patched));
            ui.label(
                egui::RichText::new(path.display().to_string())
                    .monospace()
                    .small(),
            );
        });

        ui.add_space(8.0);
        Self::render_log(ui, log);
        ui.add_space(8.0);

        ui.label(
            egui::RichText::new("To rollback later, relaunch this patcher and select the same folder.")
                .small()
                .color(egui::Color32::GRAY),
        );
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.button("Rollback").clicked() {
                self.start_rollback(path.clone(), false);
            }
            if ui.button("Quit").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }

    fn render_error(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        message: String,
        details: Option<String>,
        show_details: bool,
        log: Vec<String>,
    ) {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);

            // Red circle with white X
            let (rect, _) = ui.allocate_exact_size(egui::vec2(60.0, 60.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 30.0, egui::Color32::from_rgb(239, 68, 68));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "\u{2717}",
                egui::FontId::proportional(36.0),
                egui::Color32::WHITE,
            );

            ui.add_space(8.0);
            ui.heading("Error");
        });

        ui.add_space(4.0);
        ui.label(&message);

        if let Some(ref detail_text) = details {
            ui.add_space(4.0);
            let button_text = if show_details {
                "Hide Details"
            } else {
                "Show Details"
            };
            if ui.button(button_text).clicked() {
                self.state = AppState::Error {
                    message: message.clone(),
                    details: details.clone(),
                    show_details: !show_details,
                    log: log.clone(),
                };
            }

            if show_details {
                ui.add_space(4.0);
                egui::ScrollArea::vertical()
                    .max_height(60.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(detail_text).monospace().small());
                    });
            }
        }

        ui.add_space(8.0);
        Self::render_log(ui, &log);
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.button("Try Again").clicked() {
                self.state = AppState::Welcome;
            }
            if ui.button("Quit").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }

    fn render_validating_folder(&self, ui: &mut egui::Ui, path: &PathBuf) {
        ui.heading("Validating Folder...");
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.label("Checking:");
            ui.label(egui::RichText::new(path.display().to_string()).monospace());
        });

        ui.add_space(16.0);
        ui.spinner();
        ui.label("Verifying files can be patched...");
    }

    fn render_folder_invalid(&mut self, ui: &mut egui::Ui, path: PathBuf, reason: String) {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);

            // Orange warning circle
            let (rect, _) = ui.allocate_exact_size(egui::vec2(60.0, 60.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 30.0, egui::Color32::from_rgb(245, 158, 11));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "!",
                egui::FontId::proportional(36.0),
                egui::Color32::WHITE,
            );

            ui.add_space(8.0);
            ui.heading("Cannot Patch This Folder");
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label("Target folder:");
            ui.label(egui::RichText::new(path.display().to_string()).monospace().small());
        });

        ui.add_space(8.0);
        ui.label("Reason:");
        egui::ScrollArea::vertical()
            .max_height(80.0)
            .show(ui, |ui| {
                ui.label(egui::RichText::new(&reason).monospace().small());
            });

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("Choose Different Folder...").clicked() {
                self.select_folder();
            }
        });
    }

    fn render_already_patched(&mut self, ui: &mut egui::Ui, path: PathBuf, modified: bool) {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);

            // Blue info circle
            let (rect, _) = ui.allocate_exact_size(egui::vec2(60.0, 60.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 30.0, egui::Color32::from_rgb(59, 130, 246));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "i",
                egui::FontId::proportional(36.0),
                egui::Color32::WHITE,
            );

            ui.add_space(8.0);
            if modified {
                ui.heading("Folder Was Patched (Modified)");
            } else {
                ui.heading("Folder Already Patched");
            }
        });

        ui.add_space(8.0);
        ui.group(|ui| {
            ui.label("Target folder:");
            ui.label(egui::RichText::new(path.display().to_string()).monospace().small());
        });

        ui.add_space(8.0);
        if modified {
            ui.label("This folder was patched but files have been modified since.");
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if ui.button("Re-apply Patch").clicked() {
                    self.start_apply(path.clone());
                }
                if ui.button("Rollback Anyway").clicked() {
                    self.start_rollback(path.clone(), true);
                }
            });
        } else {
            ui.label("This folder appears to already be patched.");
            ui.add_space(16.0);
            ui.horizontal(|ui| {
                if ui.button("Rollback").clicked() {
                    self.start_rollback(path.clone(), false);
                }
            });
        }
    }

    fn render_rolling_back(&self, ui: &mut egui::Ui, path: &PathBuf, log: &[String]) {
        ui.heading("Rolling Back...");
        ui.add_space(16.0);

        ui.group(|ui| {
            ui.label("Target folder:");
            ui.label(egui::RichText::new(path.display().to_string()).monospace().small());
        });

        ui.add_space(8.0);
        ui.spinner();
        ui.add_space(8.0);
        Self::render_log(ui, log);
    }

    fn render_rollback_warning(&mut self, ui: &mut egui::Ui, path: PathBuf, reason: String) {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);

            // Orange warning circle
            let (rect, _) = ui.allocate_exact_size(egui::vec2(60.0, 60.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 30.0, egui::Color32::from_rgb(245, 158, 11));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "!",
                egui::FontId::proportional(36.0),
                egui::Color32::WHITE,
            );

            ui.add_space(8.0);
            ui.heading("Files Modified Since Patching");
        });

        ui.add_space(8.0);
        ui.label("Some files have been changed after the patch was applied.");
        ui.add_space(4.0);
        egui::ScrollArea::vertical()
            .max_height(80.0)
            .show(ui, |ui| {
                ui.label(egui::RichText::new(&reason).monospace().small());
            });

        ui.add_space(16.0);
        ui.horizontal(|ui| {
            if ui.button("Rollback Anyway").clicked() {
                self.start_rollback(path.clone(), true);
            }
            if ui.button("Cancel").clicked() {
                self.state = AppState::Welcome;
            }
        });
    }

    fn render_rollback_complete(
        &mut self,
        ctx: &egui::Context,
        ui: &mut egui::Ui,
        path: PathBuf,
        log: &[String],
    ) {
        ui.vertical_centered(|ui| {
            ui.add_space(8.0);

            // Green circle with checkmark
            let (rect, _) = ui.allocate_exact_size(egui::vec2(60.0, 60.0), egui::Sense::hover());
            ui.painter()
                .circle_filled(rect.center(), 30.0, egui::Color32::from_rgb(34, 197, 94));
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                "\u{2713}",
                egui::FontId::proportional(36.0),
                egui::Color32::WHITE,
            );

            ui.add_space(8.0);
            ui.heading("Rollback Complete");
        });

        ui.add_space(8.0);
        ui.label(
            egui::RichText::new(path.display().to_string())
                .monospace()
                .small(),
        );

        ui.add_space(8.0);
        Self::render_log(ui, log);
        ui.add_space(8.0);

        ui.label("Would you like to delete the backup files?");
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            if ui.button("Delete Backup").clicked() {
                if let Err(e) = PatchRunner::delete_backup(&path) {
                    self.state = AppState::Error {
                        message: "Failed to delete backup".to_string(),
                        details: Some(e.to_string()),
                        show_details: false,
                        log: log.to_vec(),
                    };
                } else {
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
            if ui.button("Keep Backup").clicked() {
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
        });
    }
}

impl eframe::App for GraftApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Process any pending messages
        self.process_progress_messages();
        self.process_validation_messages();
        self.process_rollback_messages();

        // Request repaint for async states (to get progress updates)
        if matches!(
            self.state,
            AppState::Applying { .. }
                | AppState::ValidatingFolder { .. }
                | AppState::RollingBack { .. }
        ) {
            ctx.request_repaint();
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.add_space(16.0);

            // Clone state to avoid borrow issues
            let state = self.state.clone();
            match state {
                AppState::Welcome => self.render_welcome(ui),
                AppState::ValidatingFolder { path } => self.render_validating_folder(ui, &path),
                AppState::FolderInvalid { path, reason } => {
                    self.render_folder_invalid(ui, path, reason)
                }
                AppState::AlreadyPatched { path, modified } => {
                    self.render_already_patched(ui, path, modified)
                }
                AppState::FolderSelected { path } => self.render_folder_selected(ui, path),
                AppState::Applying {
                    log,
                    progress,
                    current_phase,
                    ..
                } => self.render_applying(ui, log, progress, current_phase),
                AppState::Success {
                    path,
                    files_patched,
                    log,
                } => self.render_success(ctx, ui, path, files_patched, &log),
                AppState::RollingBack { path, log, .. } => {
                    self.render_rolling_back(ui, &path, &log)
                }
                AppState::RollbackWarning { path, reason } => {
                    self.render_rollback_warning(ui, path, reason)
                }
                AppState::RollbackComplete { path, log } => {
                    self.render_rollback_complete(ctx, ui, path, &log)
                }
                AppState::Error {
                    message,
                    details,
                    show_details,
                    log,
                } => self.render_error(ctx, ui, message, details, show_details, log),
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
        match GraftApp::new(data.to_vec()) {
            Ok(app) => app,
            Err(e) => {
                eprintln!("Failed to load embedded patch: {}", e);
                return Err(eframe::Error::AppCreation(Box::new(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    e.to_string(),
                ))));
            }
        }
    } else {
        GraftApp::demo()
    };

    let title = app.title.clone();
    eframe::run_native(
        &title,
        options,
        Box::new(|_cc| Ok(Box::new(app))),
    )
}
