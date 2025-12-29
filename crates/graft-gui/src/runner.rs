use flate2::read::GzDecoder;
use graft_core::patch::{self, PatchError};
use graft_core::utils::manifest::Manifest;
use std::path::{Path, PathBuf};
use tar::Archive;

/// Progress event emitted during patch application
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// Starting to process a file
    Processing { file: String, index: usize, total: usize },
    /// File processed successfully
    Processed { index: usize, total: usize },
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
    pub fn apply<F>(&self, target: &Path, mut on_progress: F) -> Result<(), PatchError>
    where
        F: FnMut(ProgressEvent),
    {
        let total = self.manifest.entries.len();

        for (i, entry) in self.manifest.entries.iter().enumerate() {
            let file = entry.file().to_string();

            on_progress(ProgressEvent::Processing {
                file: file.clone(),
                index: i,
                total,
            });

            if let Err(e) = patch::apply::apply_entry(entry, target, &self.patch_dir) {
                on_progress(ProgressEvent::Error {
                    message: format!("Failed to apply patch to '{}'", file),
                    details: Some(e.to_string()),
                });
                return Err(e);
            }

            on_progress(ProgressEvent::Processed { index: i, total });
        }

        on_progress(ProgressEvent::Done {
            files_patched: total,
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
