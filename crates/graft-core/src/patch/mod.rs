pub mod apply;
mod constants;
mod error;
pub mod validate;
pub mod verify;

// Re-export public items
pub use constants::{BACKUP_DIR, DIFFS_DIR, DIFF_EXTENSION, FILES_DIR, MANIFEST_FILENAME};
pub use error::PatchError;
pub use validate::validate_patch_dir;
