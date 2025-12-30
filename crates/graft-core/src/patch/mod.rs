pub mod apply;
pub mod backup;
mod constants;
mod error;
pub mod validate;
pub mod verify;

/// Progress information passed to callbacks during batch operations.
#[derive(Debug, Clone)]
pub struct Progress<'a> {
    /// File being processed
    pub file: &'a str,
    /// Current index (0-based)
    pub index: usize,
    /// Total number of entries
    pub total: usize,
    /// Action being performed (e.g., "Patching", "Adding", "Deleting")
    pub action: &'static str,
}

// Re-export public items
pub use apply::{apply_entries, apply_entry};
pub use backup::{backup_entries, rollback};
pub use constants::{BACKUP_DIR, DIFFS_DIR, DIFF_EXTENSION, FILES_DIR, MANIFEST_FILENAME};
pub use error::PatchError;
pub use validate::{validate_backup, validate_entries, validate_patch_dir};
pub use verify::verify_entry;
