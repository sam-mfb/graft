use std::fs;
use std::path::Path;

use crate::patch::{PatchError, DIFFS_DIR, DIFF_EXTENSION, FILES_DIR};
use crate::utils::diff::apply_diff;
use crate::utils::manifest::ManifestEntry;

/// Apply a single manifest entry to the target directory.
///
/// - Patch: reads original file, applies diff, writes result
/// - Add: copies file from patch files/ directory
/// - Delete: removes file from target directory
pub fn apply_entry(
    entry: &ManifestEntry,
    target_dir: &Path,
    patch_dir: &Path,
) -> Result<(), PatchError> {
    match entry {
        ManifestEntry::Patch { file, .. } => {
            let target_path = target_dir.join(file);
            let diff_path = patch_dir
                .join(DIFFS_DIR)
                .join(format!("{}{}", file, DIFF_EXTENSION));

            // Validate files exist before attempting operations
            if !target_path.exists() {
                return Err(PatchError::ValidationFailed {
                    file: file.clone(),
                    reason: "target file not found".to_string(),
                });
            }
            if !diff_path.exists() {
                return Err(PatchError::ValidationFailed {
                    file: file.clone(),
                    reason: "diff file not found in patch".to_string(),
                });
            }

            let original_data = fs::read(&target_path).map_err(|e| PatchError::ApplyFailed {
                file: file.clone(),
                reason: format!("failed to read original file: {}", e),
            })?;

            let diff_data = fs::read(&diff_path).map_err(|e| PatchError::ApplyFailed {
                file: file.clone(),
                reason: format!("failed to read diff file: {}", e),
            })?;

            let patched_data =
                apply_diff(&original_data, &diff_data).map_err(|e| PatchError::ApplyFailed {
                    file: file.clone(),
                    reason: format!("failed to apply diff: {}", e),
                })?;

            fs::write(&target_path, patched_data).map_err(|e| PatchError::ApplyFailed {
                file: file.clone(),
                reason: format!("failed to write patched file: {}", e),
            })?;
        }
        ManifestEntry::Add { file, .. } => {
            let source_path = patch_dir.join(FILES_DIR).join(file);
            let target_path = target_dir.join(file);

            // Validate source file exists
            if !source_path.exists() {
                return Err(PatchError::ValidationFailed {
                    file: file.clone(),
                    reason: "source file not found in patch".to_string(),
                });
            }

            fs::copy(&source_path, &target_path).map_err(|e| PatchError::ApplyFailed {
                file: file.clone(),
                reason: format!("failed to copy new file: {}", e),
            })?;
        }
        ManifestEntry::Delete { file, .. } => {
            let target_path = target_dir.join(file);

            // Only delete if file exists (already deleted is not an error)
            if target_path.exists() {
                fs::remove_file(&target_path).map_err(|e| PatchError::ApplyFailed {
                    file: file.clone(),
                    reason: format!("failed to delete file: {}", e),
                })?;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::diff::create_diff;
    use crate::utils::hash::hash_bytes;
    use tempfile::tempdir;

    #[test]
    fn apply_patch_entry() {
        let target_dir = tempdir().unwrap();
        let patch_dir = tempdir().unwrap();

        let original_content = b"original content";
        let new_content = b"modified content";

        // Set up target file
        fs::write(target_dir.path().join("file.bin"), original_content).unwrap();

        // Set up diff file
        let diff_data = create_diff(original_content, new_content).unwrap();
        fs::create_dir_all(patch_dir.path().join(DIFFS_DIR)).unwrap();
        fs::write(
            patch_dir
                .path()
                .join(DIFFS_DIR)
                .join(format!("file.bin{}", DIFF_EXTENSION)),
            &diff_data,
        )
        .unwrap();

        let entry = ManifestEntry::Patch {
            file: "file.bin".to_string(),
            original_hash: hash_bytes(original_content),
            diff_hash: hash_bytes(&diff_data),
            final_hash: hash_bytes(new_content),
        };

        apply_entry(&entry, target_dir.path(), patch_dir.path()).unwrap();

        let result = fs::read(target_dir.path().join("file.bin")).unwrap();
        assert_eq!(result, new_content);
    }

    #[test]
    fn apply_add_entry() {
        let target_dir = tempdir().unwrap();
        let patch_dir = tempdir().unwrap();

        let content = b"new file content";

        // Set up source file in patch
        fs::create_dir_all(patch_dir.path().join(FILES_DIR)).unwrap();
        fs::write(patch_dir.path().join(FILES_DIR).join("new.bin"), content).unwrap();

        let entry = ManifestEntry::Add {
            file: "new.bin".to_string(),
            final_hash: hash_bytes(content),
        };

        apply_entry(&entry, target_dir.path(), patch_dir.path()).unwrap();

        let result = fs::read(target_dir.path().join("new.bin")).unwrap();
        assert_eq!(result, content);
    }

    #[test]
    fn apply_delete_entry() {
        let target_dir = tempdir().unwrap();
        let patch_dir = tempdir().unwrap();

        let content = b"to be deleted";

        // Set up file to delete
        fs::write(target_dir.path().join("delete.bin"), content).unwrap();

        let entry = ManifestEntry::Delete {
            file: "delete.bin".to_string(),
            original_hash: hash_bytes(content),
        };

        assert!(target_dir.path().join("delete.bin").exists());
        apply_entry(&entry, target_dir.path(), patch_dir.path()).unwrap();
        assert!(!target_dir.path().join("delete.bin").exists());
    }

    #[test]
    fn apply_delete_already_missing() {
        let target_dir = tempdir().unwrap();
        let patch_dir = tempdir().unwrap();

        let entry = ManifestEntry::Delete {
            file: "already_gone.bin".to_string(),
            original_hash: "somehash".to_string(),
        };

        // Should not error if file doesn't exist
        let result = apply_entry(&entry, target_dir.path(), patch_dir.path());
        assert!(result.is_ok());
    }

    #[test]
    fn apply_patch_missing_target_returns_validation_error() {
        let target_dir = tempdir().unwrap();
        let patch_dir = tempdir().unwrap();

        // Create diff file but not target file
        fs::create_dir_all(patch_dir.path().join(DIFFS_DIR)).unwrap();
        fs::write(
            patch_dir
                .path()
                .join(DIFFS_DIR)
                .join(format!("missing.bin{}", DIFF_EXTENSION)),
            b"diff",
        )
        .unwrap();

        let entry = ManifestEntry::Patch {
            file: "missing.bin".to_string(),
            original_hash: "x".to_string(),
            diff_hash: "y".to_string(),
            final_hash: "z".to_string(),
        };

        let result = apply_entry(&entry, target_dir.path(), patch_dir.path());
        assert!(matches!(result, Err(PatchError::ValidationFailed { .. })));
    }

    #[test]
    fn apply_patch_missing_diff_returns_validation_error() {
        let target_dir = tempdir().unwrap();
        let patch_dir = tempdir().unwrap();

        // Create target file but not diff file
        fs::write(target_dir.path().join("file.bin"), b"content").unwrap();

        let entry = ManifestEntry::Patch {
            file: "file.bin".to_string(),
            original_hash: "x".to_string(),
            diff_hash: "y".to_string(),
            final_hash: "z".to_string(),
        };

        let result = apply_entry(&entry, target_dir.path(), patch_dir.path());
        assert!(matches!(result, Err(PatchError::ValidationFailed { .. })));
    }

    #[test]
    fn apply_add_missing_source_returns_validation_error() {
        let target_dir = tempdir().unwrap();
        let patch_dir = tempdir().unwrap();

        let entry = ManifestEntry::Add {
            file: "missing.bin".to_string(),
            final_hash: "x".to_string(),
        };

        let result = apply_entry(&entry, target_dir.path(), patch_dir.path());
        assert!(matches!(result, Err(PatchError::ValidationFailed { .. })));
    }
}
