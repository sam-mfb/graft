use flate2::read::GzDecoder;
use graft_core::patch::MANIFEST_FILENAME;
use graft_core::utils::manifest::Manifest;
use std::io::Read;
use tar::Archive;

// Re-export PatchInfo for use by other modules in this crate
pub use graft_core::utils::manifest::PatchInfo;

/// Validates patch data and extracts metadata without full extraction
pub struct PatchValidator;

impl PatchValidator {
    /// Validate patch data and extract info by reading only the manifest.
    /// Does NOT extract files to disk - just reads manifest from archive.
    pub fn validate(data: &[u8]) -> Result<PatchInfo, PatchValidationError> {
        let decoder = GzDecoder::new(data);
        let mut archive = Archive::new(decoder);

        let entries = archive.entries().map_err(|e| {
            PatchValidationError::DecompressionFailed(format!("Failed to read archive: {}", e))
        })?;

        for entry in entries {
            let mut entry = entry.map_err(|e| {
                PatchValidationError::DecompressionFailed(format!("Failed to read entry: {}", e))
            })?;

            let path = entry.path().map_err(|e| {
                PatchValidationError::DecompressionFailed(format!("Failed to read path: {}", e))
            })?;

            if path.ends_with(MANIFEST_FILENAME) {
                let mut content = String::new();
                entry.read_to_string(&mut content).map_err(|e| {
                    PatchValidationError::ManifestInvalid(format!("Failed to read manifest: {}", e))
                })?;

                let manifest: Manifest = serde_json::from_str(&content).map_err(|e| {
                    PatchValidationError::ManifestInvalid(format!("Invalid manifest JSON: {}", e))
                })?;

                return Ok(PatchInfo::from_manifest(&manifest));
            }
        }

        Err(PatchValidationError::ManifestNotFound)
    }
}

/// Errors from patch validation
#[derive(Debug, Clone)]
pub enum PatchValidationError {
    DecompressionFailed(String),
    ManifestNotFound,
    ManifestInvalid(String),
}

impl std::fmt::Display for PatchValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatchValidationError::DecompressionFailed(msg) => {
                write!(f, "Decompression failed: {}", msg)
            }
            PatchValidationError::ManifestNotFound => write!(f, "Manifest not found in archive"),
            PatchValidationError::ManifestInvalid(msg) => write!(f, "Invalid manifest: {}", msg),
        }
    }
}

impl std::error::Error for PatchValidationError {}
