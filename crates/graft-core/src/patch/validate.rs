use crate::patch::constants::{DIFFS_DIR, DIFF_EXTENSION, FILES_DIR, MANIFEST_FILENAME};
use crate::patch::error::PatchError;
use crate::patch::verify::verify_entry;
use crate::patch::{Progress, ProgressAction};
use crate::path_restrictions;
use crate::utils::hash::hash_bytes;
use crate::utils::manifest::{Manifest, ManifestEntry};
use std::fs;
use std::path::Path;

/// Validate that a patch directory contains all required files.
///
/// Checks that:
/// - manifest.json exists and is valid
/// - All diff files referenced by Patch entries exist
/// - All files referenced by Add entries exist
///
/// Returns the loaded Manifest on success.
pub fn validate_patch_dir(patch_dir: &Path) -> Result<Manifest, PatchError> {
    // Check manifest exists
    let manifest_path = patch_dir.join(MANIFEST_FILENAME);
    if !manifest_path.exists() {
        return Err(PatchError::ManifestNotFound);
    }

    // Load and parse manifest
    let manifest = Manifest::load(&manifest_path).map_err(|e| PatchError::ManifestError {
        reason: e.to_string(),
    })?;

    // Check all referenced files exist
    for entry in &manifest.entries {
        match entry {
            ManifestEntry::Patch { file, .. } => {
                let diff_path = patch_dir
                    .join(DIFFS_DIR)
                    .join(format!("{}{}", file, DIFF_EXTENSION));
                if !diff_path.exists() {
                    return Err(PatchError::DiffNotFound(file.clone()));
                }
            }
            ManifestEntry::Add { file, .. } => {
                let file_path = patch_dir.join(FILES_DIR).join(file);
                if !file_path.exists() {
                    return Err(PatchError::FileNotFound(file.clone()));
                }
            }
            ManifestEntry::Delete { .. } => {
                // Nothing to check - file should exist in target, not in patch
            }
        }
    }

    Ok(manifest)
}

/// Validate all manifest entries against a target directory before applying.
///
/// Checks that:
/// - For Patch entries: file exists and hash matches original_hash
/// - For Add entries: file does NOT already exist
/// - For Delete entries: if file exists, hash matches original_hash
///
/// This should be called before applying any changes to ensure the target
/// directory is in the expected state.
pub fn validate_entries<F>(
    entries: &[ManifestEntry],
    target_dir: &Path,
    mut on_progress: Option<F>,
) -> Result<(), PatchError>
where
    F: FnMut(Progress),
{
    let total = entries.len();
    for (index, entry) in entries.iter().enumerate() {
        let action = match entry {
            ManifestEntry::Patch { .. } => ProgressAction::Validating,
            ManifestEntry::Add { .. } => ProgressAction::CheckingNotExists,
            ManifestEntry::Delete { .. } => ProgressAction::Validating,
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
            ManifestEntry::Patch {
                file,
                original_hash,
                ..
            } => {
                let target_path = target_dir.join(file);

                if !target_path.exists() {
                    return Err(PatchError::ValidationFailed {
                        file: file.clone(),
                        reason: "file not found in target".to_string(),
                    });
                }

                let data = fs::read(&target_path).map_err(|e| PatchError::ValidationFailed {
                    file: file.clone(),
                    reason: format!("failed to read file: {}", e),
                })?;

                let actual_hash = hash_bytes(&data);
                if &actual_hash != original_hash {
                    return Err(PatchError::ValidationFailed {
                        file: file.clone(),
                        reason: format!(
                            "hash mismatch: expected {}, got {}",
                            original_hash, actual_hash
                        ),
                    });
                }
            }
            ManifestEntry::Add { file, .. } => {
                let target_path = target_dir.join(file);

                if target_path.exists() {
                    return Err(PatchError::ValidationFailed {
                        file: file.clone(),
                        reason: "file already exists in target".to_string(),
                    });
                }
            }
            ManifestEntry::Delete { file, original_hash } => {
                let target_path = target_dir.join(file);

                // Only validate hash if file exists - already gone is fine
                if target_path.exists() {
                    let data = fs::read(&target_path).map_err(|e| PatchError::ValidationFailed {
                        file: file.clone(),
                        reason: format!("failed to read file: {}", e),
                    })?;

                    let actual_hash = hash_bytes(&data);
                    if &actual_hash != original_hash {
                        return Err(PatchError::ValidationFailed {
                            file: file.clone(),
                            reason: format!(
                                "hash mismatch: expected {}, got {}",
                                original_hash, actual_hash
                            ),
                        });
                    }
                }
            }
        }
    }

    Ok(())
}

/// Validate that backup directory contains expected files with correct hashes.
///
/// This should be called before rolling back to ensure the backup is intact.
///
/// Checks that:
/// - For Patch entries: backup file MUST exist with hash matching original_hash
/// - For Delete entries: if backup exists, hash MUST match original_hash (missing OK)
/// - For Add entries: no backup expected
pub fn validate_backup<F>(
    entries: &[ManifestEntry],
    backup_dir: &Path,
    mut on_progress: Option<F>,
) -> Result<(), PatchError>
where
    F: FnMut(Progress),
{
    let total = entries.len();
    for (index, entry) in entries.iter().enumerate() {
        let action = match entry {
            ManifestEntry::Patch { .. } => ProgressAction::Validating,
            ManifestEntry::Add { .. } => ProgressAction::Skipping,
            ManifestEntry::Delete { .. } => ProgressAction::Validating,
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
            ManifestEntry::Patch {
                file,
                original_hash,
                ..
            } => {
                let backup_path = backup_dir.join(file);
                if !backup_path.exists() {
                    return Err(PatchError::RollbackFailed {
                        reason: format!("backup file not found: {}", file),
                    });
                }
                let data = fs::read(&backup_path).map_err(|e| PatchError::RollbackFailed {
                    reason: format!("failed to read backup '{}': {}", file, e),
                })?;
                let actual_hash = hash_bytes(&data);
                if &actual_hash != original_hash {
                    return Err(PatchError::RollbackFailed {
                        reason: format!(
                            "backup hash mismatch for '{}': expected {}, got {}",
                            file, original_hash, actual_hash
                        ),
                    });
                }
            }
            ManifestEntry::Delete { file, original_hash } => {
                let backup_path = backup_dir.join(file);
                if backup_path.exists() {
                    let data = fs::read(&backup_path).map_err(|e| PatchError::RollbackFailed {
                        reason: format!("failed to read backup '{}': {}", file, e),
                    })?;
                    let actual_hash = hash_bytes(&data);
                    if &actual_hash != original_hash {
                        return Err(PatchError::RollbackFailed {
                            reason: format!(
                                "backup hash mismatch for '{}': expected {}, got {}",
                                file, original_hash, actual_hash
                            ),
                        });
                    }
                }
            }
            ManifestEntry::Add { .. } => {
                // No backup for added files
            }
        }
    }
    Ok(())
}

/// Validate that all entries are in their expected post-patch state.
///
/// This verifies:
/// - Patch entries: file exists and matches final_hash
/// - Add entries: file exists and matches final_hash
/// - Delete entries: file does not exist
///
/// Use this before rollback to ensure patched files haven't been modified,
/// or after apply to confirm patches were applied correctly.
pub fn validate_patched_entries<F>(
    entries: &[ManifestEntry],
    target_dir: &Path,
    mut on_progress: Option<F>,
) -> Result<(), PatchError>
where
    F: FnMut(Progress),
{
    let total = entries.len();
    for (index, entry) in entries.iter().enumerate() {
        if let Some(ref mut callback) = on_progress {
            callback(Progress {
                file: entry.file(),
                index,
                total,
                action: ProgressAction::Validating,
            });
        }

        verify_entry(entry, target_dir)?;
    }
    Ok(())
}

/// Validate that a manifest's paths don't violate security restrictions.
///
/// When `manifest.allow_restricted` is false (the default), this checks:
/// - No path traversal sequences (../)
/// - No protected system directories
/// - No blocked file extensions (executables)
///
/// If `manifest.allow_restricted` is true, all checks are bypassed.
pub fn validate_path_restrictions(
    manifest: &Manifest,
    target_dir: &Path,
) -> Result<(), PatchError> {
    path_restrictions::check_manifest(manifest, target_dir)
        .map_err(PatchError::RestrictedPaths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn validates_empty_patch() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("manifest.json"),
            r#"{"version": 1, "entries": []}"#,
        )
        .unwrap();

        let result = validate_patch_dir(dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn fails_without_manifest() {
        let dir = tempdir().unwrap();
        let result = validate_patch_dir(dir.path());
        assert!(matches!(result, Err(PatchError::ManifestNotFound)));
    }

    #[test]
    fn fails_with_missing_diff() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("manifest.json"),
            r#"{"version": 1, "entries": [
                {"operation": "patch", "file": "test.bin", "original_hash": "a", "diff_hash": "b", "final_hash": "c"}
            ]}"#,
        )
        .unwrap();

        let result = validate_patch_dir(dir.path());
        assert!(matches!(result, Err(PatchError::DiffNotFound(_))));
    }

    #[test]
    fn fails_with_missing_add_file() {
        let dir = tempdir().unwrap();
        fs::write(
            dir.path().join("manifest.json"),
            r#"{"version": 1, "entries": [
                {"operation": "add", "file": "new.bin", "final_hash": "a"}
            ]}"#,
        )
        .unwrap();

        let result = validate_patch_dir(dir.path());
        assert!(matches!(result, Err(PatchError::FileNotFound(_))));
    }

    #[test]
    fn validates_complete_patch() {
        let dir = tempdir().unwrap();

        // Create manifest
        fs::write(
            dir.path().join("manifest.json"),
            r#"{"version": 1, "entries": [
                {"operation": "patch", "file": "modified.bin", "original_hash": "a", "diff_hash": "b", "final_hash": "c"},
                {"operation": "add", "file": "new.bin", "final_hash": "d"},
                {"operation": "delete", "file": "old.bin", "original_hash": "e"}
            ]}"#,
        )
        .unwrap();

        // Create diffs directory and diff file
        fs::create_dir(dir.path().join("diffs")).unwrap();
        fs::write(dir.path().join("diffs/modified.bin.diff"), b"diff data").unwrap();

        // Create files directory and new file
        fs::create_dir(dir.path().join("files")).unwrap();
        fs::write(dir.path().join("files/new.bin"), b"new file data").unwrap();

        let result = validate_patch_dir(dir.path());
        assert!(result.is_ok());

        let manifest = result.unwrap();
        assert_eq!(manifest.entries.len(), 3);
    }
}
