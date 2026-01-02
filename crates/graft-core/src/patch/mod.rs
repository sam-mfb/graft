pub mod apply;
pub mod backup;
mod constants;
mod error;
pub mod validate;
pub mod verify;

/// Action being performed on a file during progress.
///
/// Consumers can format this enum however they want for display or localization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProgressAction {
    // Validation phase
    Validating,
    CheckingNotExists,

    // Backup phase
    BackingUp,
    Skipping,

    // Apply phase
    Patching,
    Adding,
    Deleting,

    // Rollback phase
    Restoring,
    Removing,
}

/// Progress information passed to callbacks during batch operations.
#[derive(Debug, Clone)]
pub struct Progress<'a> {
    /// File being processed
    pub file: &'a str,
    /// Current index (0-based)
    pub index: usize,
    /// Total number of entries
    pub total: usize,
    /// Action being performed
    pub action: ProgressAction,
}

// Re-export public items
pub use apply::{apply_entries, apply_entry};
pub use backup::{backup_entries, rollback};
pub use constants::{ASSETS_DIR, BACKUP_DIR, DIFFS_DIR, DIFF_EXTENSION, FILES_DIR, ICON_FILENAME, MANIFEST_FILENAME};
pub use error::PatchError;
pub use validate::{validate_backup, validate_entries, validate_patch_dir, validate_patched_entries, validate_path_restrictions};
pub use verify::verify_entry;
