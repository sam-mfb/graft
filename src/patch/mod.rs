pub mod apply;
pub mod verify;

use std::fmt;

/// Directory name for diff files within a patch
pub const DIFFS_DIR: &str = "diffs";
/// Directory name for new files within a patch
pub const FILES_DIR: &str = "files";
/// File extension for diff files
pub const DIFF_EXTENSION: &str = ".diff";
/// Filename for the manifest
pub const MANIFEST_FILENAME: &str = "manifest.json";
/// Directory name for backups during patch application
pub const BACKUP_DIR: &str = ".patch-backup";

/// Error type for patch operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchError {
    ValidationFailed { file: String, reason: String },
    BackupFailed { file: String, reason: String },
    ApplyFailed { file: String, reason: String },
    VerificationFailed { file: String, expected: String, actual: String },
    RollbackFailed { reason: String },
    ManifestError { reason: String },
}

impl fmt::Display for PatchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
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
        }
    }
}

impl std::error::Error for PatchError {}
