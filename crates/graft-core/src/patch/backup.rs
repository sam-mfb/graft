//! Backup and rollback operations for patch application.

use std::fs;
use std::path::Path;

use crate::patch::Progress;
use crate::patch::PatchError;
use crate::utils::file_ops::{backup_file, restore_file};
use crate::utils::manifest::ManifestEntry;

/// Backup all files that will be modified or deleted.
///
/// Creates a backup directory and copies files that will be changed by the patch.
/// This should be called after validation but before applying any changes.
///
/// - Patch entries: backs up the original file
/// - Delete entries: backs up the file (if it exists)
/// - Add entries: nothing to backup (new files)
pub fn backup_entries<F>(
    entries: &[ManifestEntry],
    target_dir: &Path,
    backup_dir: &Path,
    mut on_progress: Option<F>,
) -> Result<(), PatchError>
where
    F: FnMut(Progress),
{
    let total = entries.len();
    for (index, entry) in entries.iter().enumerate() {
        let action = match entry {
            ManifestEntry::Patch { .. } | ManifestEntry::Delete { .. } => "Backing up",
            ManifestEntry::Add { .. } => "Skipping",
        };

        if let Some(ref mut callback) = on_progress {
            callback(Progress {
                file: entry.file(),
                index,
                total,
                action,
            });
        }
        match entry {
            ManifestEntry::Patch { file, .. } | ManifestEntry::Delete { file, .. } => {
                let target_path = target_dir.join(file);

                // Only backup if file exists (delete entries may already be gone)
                if target_path.exists() {
                    backup_file(&target_path, backup_dir).map_err(|e| PatchError::BackupFailed {
                        file: file.clone(),
                        reason: e.to_string(),
                    })?;
                }
            }
            ManifestEntry::Add { .. } => {
                // Nothing to backup for new files
            }
        }
    }

    Ok(())
}

/// Rollback applied changes by restoring from backup and removing added files.
///
/// This should be called when an error occurs during patch application to
/// restore the target directory to its original state.
///
/// - Patch entries: restores the original file from backup
/// - Delete entries: restores the file from backup (if backup exists)
/// - Add entries: removes the newly added file
pub fn rollback<F>(
    applied: &[&ManifestEntry],
    target_dir: &Path,
    backup_dir: &Path,
    mut on_progress: Option<F>,
) -> Result<(), PatchError>
where
    F: FnMut(Progress),
{
    let total = applied.len();
    for (index, entry) in applied.iter().enumerate() {
        let action = match entry {
            ManifestEntry::Patch { .. } => "Restoring",
            ManifestEntry::Add { .. } => "Removing",
            ManifestEntry::Delete { .. } => "Restoring",
        };

        if let Some(ref mut callback) = on_progress {
            callback(Progress {
                file: entry.file(),
                index,
                total,
                action,
            });
        }
        match entry {
            ManifestEntry::Patch { file, .. } => {
                // Patch entries always have backups (validated to exist)
                let target_path = target_dir.join(file);
                restore_file(&target_path, backup_dir).map_err(|e| PatchError::RollbackFailed {
                    reason: format!("failed to restore '{}': {}", file, e),
                })?;
            }
            ManifestEntry::Delete { file, .. } => {
                // Only restore if we have a backup (file existed before patch)
                let backup_path = backup_dir.join(file);
                if backup_path.exists() {
                    let target_path = target_dir.join(file);
                    restore_file(&target_path, backup_dir).map_err(|e| {
                        PatchError::RollbackFailed {
                            reason: format!("failed to restore '{}': {}", file, e),
                        }
                    })?;
                }
            }
            ManifestEntry::Add { file, .. } => {
                // Remove the newly added file
                let target_path = target_dir.join(file);
                if target_path.exists() {
                    fs::remove_file(&target_path).map_err(|e| PatchError::RollbackFailed {
                        reason: format!("failed to remove added file '{}': {}", file, e),
                    })?;
                }
            }
        }
    }

    Ok(())
}
