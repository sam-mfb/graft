use flate2::read::GzDecoder;
use graft_core::patch::{self, PatchError, Progress};
use graft_core::utils::manifest::Manifest;
use std::cell::RefCell;
use std::fmt;
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
        action: String,
    },
    /// Patch completed successfully
    Done { files_patched: usize },
    /// An error occurred
    Error { message: String, details: Option<String> },
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
                action: p.action.to_owned(),
            });
        };

        // Validation phase
        (on_progress.borrow_mut())(ProgressEvent::PhaseStarted {
            phase: Phase::Validating,
        });
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
