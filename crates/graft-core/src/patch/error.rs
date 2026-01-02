use crate::path_restrictions::RestrictionViolation;
use std::fmt;

/// Error type for patch operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchError {
    /// Manifest file not found in patch directory
    ManifestNotFound,
    /// Diff file referenced in manifest not found
    DiffNotFound(String),
    /// File referenced in manifest not found
    FileNotFound(String),
    /// Validation failed for a file
    ValidationFailed { file: String, reason: String },
    /// Backup failed for a file
    BackupFailed { file: String, reason: String },
    /// Apply failed for a file
    ApplyFailed { file: String, reason: String },
    /// Verification failed - hash mismatch
    VerificationFailed { file: String, expected: String, actual: String },
    /// Rollback failed
    RollbackFailed { reason: String },
    /// Error with manifest
    ManifestError { reason: String },
    /// Path restrictions violated (system dirs, executables, etc.)
    RestrictedPaths(Vec<RestrictionViolation>),
}

impl fmt::Display for PatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PatchError::ManifestNotFound => {
                write!(f, "manifest.json not found in patch directory")
            }
            PatchError::DiffNotFound(file) => {
                write!(f, "diff file not found for '{}'", file)
            }
            PatchError::FileNotFound(file) => {
                write!(f, "file not found: '{}'", file)
            }
            PatchError::ValidationFailed { file, reason } => {
                write!(f, "validation failed for '{}': {}", file, reason)
            }
            PatchError::BackupFailed { file, reason } => {
                write!(f, "backup failed for '{}': {}", file, reason)
            }
            PatchError::ApplyFailed { file, reason } => {
                write!(f, "apply failed for '{}': {}", file, reason)
            }
            PatchError::VerificationFailed { file, expected, actual } => {
                write!(
                    f,
                    "verification failed for '{}': expected hash {}, got {}",
                    file, expected, actual
                )
            }
            PatchError::RollbackFailed { reason } => {
                write!(f, "rollback failed: {}", reason)
            }
            PatchError::ManifestError { reason } => {
                write!(f, "manifest error: {}", reason)
            }
            PatchError::RestrictedPaths(violations) => {
                writeln!(f, "cannot patch restricted paths:")?;
                for v in violations {
                    writeln!(f, "  - {}", v)?;
                }
                write!(
                    f,
                    "Set \"allow_restricted\": true in manifest.json to bypass (for trusted patches only)."
                )
            }
        }
    }
}

impl std::error::Error for PatchError {}
