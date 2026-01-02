use flate2::read::GzDecoder;
use graft_core::patch::{self, PatchError, Progress, BACKUP_DIR};
use graft_core::utils::manifest::Manifest;
use std::cell::RefCell;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};
use tar::Archive;

/// Processing phases for orchestration
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Validating,
    BackingUp,
    Applying,
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Phase::Validating => write!(f, "Validating"),
            Phase::BackingUp => write!(f, "Backing up"),
            Phase::Applying => write!(f, "Applying"),
        }
    }
}

/// Progress event emitted during patch application
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// A processing phase has started
    PhaseStarted { phase: Phase },
    /// Progress on a specific file operation (mapped from core Progress)
    Operation {
        file: String,
        index: usize,
        total: usize,
        action: ProgressAction,
    },
    /// Patch completed successfully
    Done { files_patched: usize },
    /// An error occurred
    Error { message: String, details: Option<String> },
}

// Re-export ProgressAction for consumers
pub use graft_core::patch::ProgressAction;

/// Progress event emitted during rollback
#[derive(Debug, Clone)]
pub enum RollbackEvent {
    /// Validating target files (patched state)
    ValidatingTarget,
    /// Target validation failed - files have been modified
    TargetModified { reason: String },
    /// Validating backup files
    ValidatingBackup,
    /// Rolling back a specific file
    Rolling {
        file: String,
        index: usize,
        total: usize,
        action: ProgressAction,
    },
    /// Rollback completed successfully
    Done { files_restored: usize },
    /// An error occurred during rollback
    Error { message: String },
}

/// Core patch runner that handles extraction and application
pub struct PatchRunner {
    patch_dir: PathBuf,
    manifest: Manifest,
}

impl PatchRunner {
    /// Create a new runner from compressed patch data
    pub fn new(data: &[u8]) -> Result<Self, PatchRunnerError> {
        // Create temp directory for extracted patch
        let temp_dir = tempfile::tempdir()
            .map_err(|e| PatchRunnerError::ExtractionFailed(format!("Failed to create temp directory: {}", e)))?;

        // Decompress and extract
        let decoder = GzDecoder::new(data);
        let mut archive = Archive::new(decoder);
        archive
            .unpack(temp_dir.path())
            .map_err(|e| PatchRunnerError::ExtractionFailed(format!("Failed to extract patch archive: {}", e)))?;

        // Load manifest
        let manifest_path = temp_dir.path().join(patch::MANIFEST_FILENAME);
        let manifest = Manifest::load(&manifest_path)
            .map_err(|e| PatchRunnerError::ManifestLoadFailed(format!("Failed to load manifest: {}", e)))?;

        // Keep temp_dir alive by converting to path
        let patch_dir = temp_dir.keep();

        Ok(PatchRunner {
            patch_dir,
            manifest,
        })
    }

    /// Apply patch to target directory with progress callback
    ///
    /// The callback is invoked for each progress event. Returns Ok(()) on success,
    /// or the first error encountered.
    ///
    /// This uses the full patch workflow including:
    /// - Validation before making any changes
    /// - Backup of files that will be modified/deleted (to .patch-backup)
    /// - Atomic rollback on failure
    pub fn apply<F>(&self, target: &Path, on_progress: F) -> Result<(), PatchError>
    where
        F: FnMut(ProgressEvent),
    {
        let backup_dir = target.join(patch::BACKUP_DIR);

        // Use RefCell to allow multiple closures to borrow on_progress
        let on_progress = RefCell::new(on_progress);

        // Helper to convert core Progress to ProgressEvent::Operation
        let send_operation = |p: Progress| {
            (on_progress.borrow_mut())(ProgressEvent::Operation {
                file: p.file.to_owned(),
                index: p.index,
                total: p.total,
                action: p.action,
            });
        };

        // Validation phase
        (on_progress.borrow_mut())(ProgressEvent::PhaseStarted {
            phase: Phase::Validating,
        });

        // Check path restrictions first (unless allow_restricted is set in manifest)
        if let Err(e) = patch::validate_path_restrictions(&self.manifest, target) {
            (on_progress.borrow_mut())(ProgressEvent::Error {
                message: "Path restrictions violated".to_string(),
                details: Some(e.to_string()),
            });
            return Err(e);
        }

        if let Err(e) = patch::validate_entries(&self.manifest.entries, target, Some(&send_operation))
        {
            (on_progress.borrow_mut())(ProgressEvent::Error {
                message: "Validation failed".to_string(),
                details: Some(e.to_string()),
            });
            return Err(e);
        }

        // Backup phase
        (on_progress.borrow_mut())(ProgressEvent::PhaseStarted {
            phase: Phase::BackingUp,
        });
        if let Err(e) =
            patch::backup_entries(&self.manifest.entries, target, &backup_dir, Some(&send_operation))
        {
            (on_progress.borrow_mut())(ProgressEvent::Error {
                message: "Backup failed".to_string(),
                details: Some(e.to_string()),
            });
            return Err(e);
        }

        // Apply phase
        (on_progress.borrow_mut())(ProgressEvent::PhaseStarted {
            phase: Phase::Applying,
        });
        if let Err(e) = patch::apply_entries(
            &self.manifest.entries,
            target,
            &self.patch_dir,
            &backup_dir,
            Some(&send_operation),
        ) {
            (on_progress.borrow_mut())(ProgressEvent::Error {
                message: "Apply failed".to_string(),
                details: Some(e.to_string()),
            });
            return Err(e);
        }

        (on_progress.borrow_mut())(ProgressEvent::Done {
            files_patched: self.manifest.entries.len(),
        });

        Ok(())
    }

    /// Validate that target folder can be patched (pre-apply check)
    ///
    /// Returns Ok(()) if all files are in expected pre-patch state,
    /// or an error describing the first problem found.
    ///
    /// Also checks path restrictions (unless allow_restricted is set in manifest).
    pub fn validate_target(&self, target: &Path) -> Result<(), PatchError> {
        // Check path restrictions first
        patch::validate_path_restrictions(&self.manifest, target)?;
        patch::validate_entries(&self.manifest.entries, target, None::<fn(Progress)>)
    }

    /// Check if target appears to be in patched state
    ///
    /// Returns true if all files match their expected post-patch hashes.
    pub fn is_patched(&self, target: &Path) -> bool {
        patch::validate_patched_entries(&self.manifest.entries, target, None::<fn(Progress)>).is_ok()
    }

    /// Check if backup directory exists in target
    pub fn has_backup(target: &Path) -> bool {
        target.join(BACKUP_DIR).exists()
    }

    /// Perform rollback with validation and progress reporting
    ///
    /// If `force` is false, validates that target files are in expected patched state first.
    /// If target files have been modified, returns TargetModified event and does not rollback.
    /// Always validates backup integrity before proceeding.
    pub fn rollback<F>(&self, target: &Path, force: bool, mut on_progress: F) -> Result<(), PatchError>
    where
        F: FnMut(RollbackEvent),
    {
        let backup_dir = target.join(BACKUP_DIR);

        // Check backup exists
        if !backup_dir.exists() {
            on_progress(RollbackEvent::Error {
                message: format!("Backup directory not found: {}", backup_dir.display()),
            });
            return Err(PatchError::RollbackFailed {
                reason: "backup directory not found".to_string(),
            });
        }

        // Validate target (patched files) unless force
        if !force {
            on_progress(RollbackEvent::ValidatingTarget);
            if let Err(e) = patch::validate_patched_entries(
                &self.manifest.entries,
                target,
                None::<fn(Progress)>,
            ) {
                on_progress(RollbackEvent::TargetModified {
                    reason: e.to_string(),
                });
                return Err(e);
            }
        }

        // Always validate backup integrity
        on_progress(RollbackEvent::ValidatingBackup);
        if let Err(e) = patch::validate_backup(&self.manifest.entries, &backup_dir, None::<fn(Progress)>) {
            on_progress(RollbackEvent::Error {
                message: format!("Backup validation failed: {}", e),
            });
            return Err(e);
        }

        // Perform rollback
        let entries: Vec<_> = self.manifest.entries.iter().collect();
        let total = entries.len();
        patch::rollback(&entries, target, &backup_dir, Some(|p: Progress| {
            on_progress(RollbackEvent::Rolling {
                file: p.file.to_owned(),
                index: p.index,
                total: p.total,
                action: p.action,
            });
        }))?;

        on_progress(RollbackEvent::Done {
            files_restored: total,
        });

        Ok(())
    }

    /// Delete the backup directory
    pub fn delete_backup(target: &Path) -> std::io::Result<()> {
        let backup_dir = target.join(BACKUP_DIR);
        if backup_dir.exists() {
            fs::remove_dir_all(&backup_dir)?;
        }
        Ok(())
    }
}

/// Errors specific to the patch runner
#[derive(Debug, Clone)]
pub enum PatchRunnerError {
    ExtractionFailed(String),
    ManifestLoadFailed(String),
}

impl std::fmt::Display for PatchRunnerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchRunnerError::ExtractionFailed(msg) => write!(f, "Extraction failed: {}", msg),
            PatchRunnerError::ManifestLoadFailed(msg) => write!(f, "Manifest load failed: {}", msg),
        }
    }
}

impl std::error::Error for PatchRunnerError {}
