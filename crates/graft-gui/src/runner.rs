use flate2::read::GzDecoder;
use graft_core::patch::{apply::apply_entry, PatchError, MANIFEST_FILENAME};
use graft_core::utils::manifest::{Manifest, ManifestEntry};
use std::path::{Path, PathBuf};
use tar::Archive;

/// Progress event emitted during patch application
#[derive(Debug, Clone)]
pub enum ProgressEvent {
    /// Starting to process a file
    Processing { file: String, index: usize, total: usize },
    /// Patch completed successfully
    Done { files_patched: usize },
    /// An error occurred
    Error { message: String, details: Option<String> },
}

/// Patch metadata extracted from manifest
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
}

/// Mock patch info for demo mode
pub fn mock_info() -> PatchInfo {
    PatchInfo {
        version: 1,
        entry_count: 42,
        patches: 35,
        additions: 5,
        deletions: 2,
    }
}

/// Core patch runner that handles extraction and application
pub struct PatchRunner {
    patch_dir: PathBuf,
    manifest: Manifest,
    info: PatchInfo,
}

impl PatchRunner {
    /// Extract patch from compressed tar.gz data
    pub fn extract(data: &[u8]) -> Result<Self, PatchRunnerError> {
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
        let manifest_path = temp_dir.path().join(MANIFEST_FILENAME);
        let manifest = Manifest::load(&manifest_path)
            .map_err(|e| PatchRunnerError::ManifestLoadFailed(format!("Failed to load manifest: {}", e)))?;

        let info = PatchInfo::from_manifest(&manifest);

        // Keep temp_dir alive by converting to path
        let patch_dir = temp_dir.keep();

        Ok(PatchRunner {
            patch_dir,
            manifest,
            info,
        })
    }

    /// Get patch metadata
    pub fn info(&self) -> &PatchInfo {
        &self.info
    }

    /// Get the manifest
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Get the patch directory path
    pub fn patch_dir(&self) -> &Path {
        &self.patch_dir
    }

    /// Apply patch to target directory with progress callback
    ///
    /// The callback is invoked for each progress event. Returns Ok(()) on success,
    /// or the first error encountered.
    #[allow(dead_code)] // Used by CLI when embedded_patch feature is enabled
    pub fn apply<F>(&self, target: &Path, on_progress: F) -> Result<(), PatchError>
    where
        F: FnMut(ProgressEvent),
    {
        apply_patch(&self.manifest, &self.patch_dir, target, on_progress)
    }
}

/// Apply a patch with progress callback (standalone function for use from threads)
///
/// This function can be used directly when you need to apply a patch from a
/// background thread and have already cloned the necessary data.
pub fn apply_patch<F>(
    manifest: &Manifest,
    patch_dir: &Path,
    target: &Path,
    mut on_progress: F,
) -> Result<(), PatchError>
where
    F: FnMut(ProgressEvent),
{
    let total = manifest.entries.len();

    for (i, entry) in manifest.entries.iter().enumerate() {
        let file = entry.file().to_string();

        on_progress(ProgressEvent::Processing {
            file: file.clone(),
            index: i,
            total,
        });

        if let Err(e) = apply_entry(entry, target, patch_dir) {
            on_progress(ProgressEvent::Error {
                message: format!("Failed to apply patch to '{}'", file),
                details: Some(e.to_string()),
            });
            return Err(e);
        }
    }

    on_progress(ProgressEvent::Done {
        files_patched: total,
    });

    Ok(())
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
