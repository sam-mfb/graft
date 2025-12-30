use std::path::Path;

use graft_core::patch::{rollback, validate_backup, PatchError, Progress, BACKUP_DIR};
use graft_core::utils::manifest::Manifest;

/// Rollback a previously applied patch using the backup directory.
///
/// This restores files from `.patch-backup` to their original state.
pub fn run(target_dir: &Path, manifest_path: &Path) -> Result<(), PatchError> {
    // Load manifest
    let manifest = Manifest::load(manifest_path).map_err(|e| PatchError::ManifestError {
        reason: e.to_string(),
    })?;

    // Get backup directory
    let backup_dir = target_dir.join(BACKUP_DIR);
    if !backup_dir.exists() {
        return Err(PatchError::RollbackFailed {
            reason: format!("backup directory not found: {}", backup_dir.display()),
        });
    }

    // Validate backup integrity before rolling back
    validate_backup(&manifest.entries, &backup_dir, Some(|p: Progress| {
        println!("{} [{}/{}]: {}", p.action, p.index + 1, p.total, p.file);
    }))?;

    // Rollback all entries (treat all as "applied")
    let entries: Vec<_> = manifest.entries.iter().collect();
    rollback(&entries, target_dir, &backup_dir, Some(|p: Progress| {
        println!("{} [{}/{}]: {}", p.action, p.index + 1, p.total, p.file);
    }))?;

    Ok(())
}
