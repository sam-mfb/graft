use crate::patch::constants::{DIFFS_DIR, DIFF_EXTENSION, FILES_DIR, MANIFEST_FILENAME};
use crate::patch::error::PatchError;
use crate::utils::manifest::{Manifest, ManifestEntry};
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
