pub mod apply;
pub mod backup;
mod constants;
mod error;
pub mod validate;
pub mod verify;

// Re-export public items
pub use apply::apply_entry;
pub use backup::{backup_entries, rollback};
pub use constants::{BACKUP_DIR, DIFFS_DIR, DIFF_EXTENSION, FILES_DIR, MANIFEST_FILENAME};
pub use error::PatchError;
pub use validate::{validate_backup, validate_entries, validate_patch_dir};
pub use verify::verify_entry;
