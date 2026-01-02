use std::path::Path;

use graft_core::patch::{
    apply_entries, backup_entries, validate_entries, validate_path_restrictions, PatchError,
    Progress, ProgressAction, BACKUP_DIR, MANIFEST_FILENAME,
};
use graft_core::utils::manifest::Manifest;

fn format_action(action: ProgressAction) -> &'static str {
    match action {
        ProgressAction::Validating => "Validating",
        ProgressAction::CheckingNotExists => "Checking",
        ProgressAction::BackingUp => "Backing up",
        ProgressAction::Skipping => "Skipping",
        ProgressAction::Patching => "Patching",
        ProgressAction::Adding => "Adding",
        ProgressAction::Deleting => "Deleting",
        ProgressAction::Restoring => "Restoring",
        ProgressAction::Removing => "Removing",
    }
}

/// Apply a patch to a target directory.
///
/// Workflow:
/// 1. Load and parse manifest
/// 2. Validate all entries (files exist, hashes match)
/// 3. Backup all files that will be modified/deleted
/// 4. Apply each entry, verifying immediately after
/// 5. On any failure, rollback to original state
pub fn run(target_dir: &Path, patch_dir: &Path) -> Result<(), PatchError> {
    // Load manifest
    let manifest_path = patch_dir.join(MANIFEST_FILENAME);
    let manifest = Manifest::load(&manifest_path).map_err(|e| PatchError::ManifestError {
        reason: e.to_string(),
    })?;

    // Check path restrictions (unless allow_restricted is set in manifest)
    validate_path_restrictions(&manifest, target_dir)?;

    // Validate all entries before making any changes
    validate_entries(&manifest.entries, target_dir, Some(|p: Progress| {
        println!("{} [{}/{}]: {}", format_action(p.action), p.index + 1, p.total, p.file);
    }))?;

    // Backup all files that will be modified/deleted
    let backup_dir = target_dir.join(BACKUP_DIR);
    backup_entries(&manifest.entries, target_dir, &backup_dir, Some(|p: Progress| {
        println!("{} [{}/{}]: {}", format_action(p.action), p.index + 1, p.total, p.file);
    }))?;

    // Apply each entry with automatic rollback on failure
    apply_entries(&manifest.entries, target_dir, patch_dir, &backup_dir, Some(|p: Progress| {
        println!("{} [{}/{}]: {}", format_action(p.action), p.index + 1, p.total, p.file);
    }))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::patch_create;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn successful_apply_modifies_target() {
        let orig_dir = tempdir().unwrap();
        let new_dir = tempdir().unwrap();
        let patch_dir = tempdir().unwrap();
        let target_dir = tempdir().unwrap();

        // Set up original and new directories
        fs::write(orig_dir.path().join("modified.bin"), b"original").unwrap();
        fs::write(new_dir.path().join("modified.bin"), b"modified").unwrap();
        fs::write(new_dir.path().join("added.bin"), b"new file").unwrap();
        fs::write(orig_dir.path().join("deleted.bin"), b"to delete").unwrap();

        // Create patch
        patch_create::run(orig_dir.path(), new_dir.path(), patch_dir.path(), 1, None, true).unwrap();

        // Set up target (copy of original)
        fs::write(target_dir.path().join("modified.bin"), b"original").unwrap();
        fs::write(target_dir.path().join("deleted.bin"), b"to delete").unwrap();

        // Apply patch
        run(target_dir.path(), patch_dir.path()).unwrap();

        // Verify results
        assert_eq!(
            fs::read(target_dir.path().join("modified.bin")).unwrap(),
            b"modified"
        );
        assert_eq!(
            fs::read(target_dir.path().join("added.bin")).unwrap(),
            b"new file"
        );
        assert!(!target_dir.path().join("deleted.bin").exists());
    }

    #[test]
    fn validation_rejects_missing_file() {
        let orig_dir = tempdir().unwrap();
        let new_dir = tempdir().unwrap();
        let patch_dir = tempdir().unwrap();
        let target_dir = tempdir().unwrap();

        // Create a patch that modifies a file
        fs::write(orig_dir.path().join("file.bin"), b"original").unwrap();
        fs::write(new_dir.path().join("file.bin"), b"modified").unwrap();
        patch_create::run(orig_dir.path(), new_dir.path(), patch_dir.path(), 1, None, true).unwrap();

        // Target is missing the file
        let result = run(target_dir.path(), patch_dir.path());

        assert!(matches!(result, Err(PatchError::ValidationFailed { .. })));
    }

    #[test]
    fn validation_rejects_hash_mismatch() {
        let orig_dir = tempdir().unwrap();
        let new_dir = tempdir().unwrap();
        let patch_dir = tempdir().unwrap();
        let target_dir = tempdir().unwrap();

        // Create a patch
        fs::write(orig_dir.path().join("file.bin"), b"original").unwrap();
        fs::write(new_dir.path().join("file.bin"), b"modified").unwrap();
        patch_create::run(orig_dir.path(), new_dir.path(), patch_dir.path(), 1, None, true).unwrap();

        // Target has different content
        fs::write(target_dir.path().join("file.bin"), b"different").unwrap();

        let result = run(target_dir.path(), patch_dir.path());

        assert!(matches!(result, Err(PatchError::ValidationFailed { .. })));
    }

    #[test]
    fn validation_rejects_existing_add_target() {
        let orig_dir = tempdir().unwrap();
        let new_dir = tempdir().unwrap();
        let patch_dir = tempdir().unwrap();
        let target_dir = tempdir().unwrap();

        // Create a patch that adds a file
        fs::write(new_dir.path().join("new.bin"), b"new content").unwrap();
        patch_create::run(orig_dir.path(), new_dir.path(), patch_dir.path(), 1, None, true).unwrap();

        // Target already has that file
        fs::write(target_dir.path().join("new.bin"), b"existing").unwrap();

        let result = run(target_dir.path(), patch_dir.path());

        assert!(matches!(result, Err(PatchError::ValidationFailed { .. })));
    }

    #[test]
    fn already_deleted_file_succeeds() {
        let orig_dir = tempdir().unwrap();
        let new_dir = tempdir().unwrap();
        let patch_dir = tempdir().unwrap();
        let target_dir = tempdir().unwrap();

        // Create a patch that deletes a file
        fs::write(orig_dir.path().join("deleted.bin"), b"content").unwrap();
        patch_create::run(orig_dir.path(), new_dir.path(), patch_dir.path(), 1, None, true).unwrap();

        // Target doesn't have the file (already deleted)
        let result = run(target_dir.path(), patch_dir.path());

        assert!(result.is_ok());
    }

    #[test]
    fn rollback_restores_on_failure() {
        let orig_dir = tempdir().unwrap();
        let new_dir = tempdir().unwrap();
        let patch_dir = tempdir().unwrap();
        let target_dir = tempdir().unwrap();

        // Create patch with two modifications
        fs::write(orig_dir.path().join("a.bin"), b"original a").unwrap();
        fs::write(new_dir.path().join("a.bin"), b"modified a").unwrap();
        fs::write(orig_dir.path().join("b.bin"), b"original b").unwrap();
        fs::write(new_dir.path().join("b.bin"), b"modified b").unwrap();
        patch_create::run(orig_dir.path(), new_dir.path(), patch_dir.path(), 1, None, true).unwrap();

        // Set up target correctly for first file, but corrupt the diff for second
        fs::write(target_dir.path().join("a.bin"), b"original a").unwrap();
        fs::write(target_dir.path().join("b.bin"), b"original b").unwrap();

        // Corrupt the second diff file to cause apply failure
        let diffs_dir = patch_dir.path().join("diffs");
        fs::write(diffs_dir.join("b.bin.diff"), b"corrupted diff data").unwrap();

        let result = run(target_dir.path(), patch_dir.path());

        // Should fail
        assert!(result.is_err());

        // First file should be rolled back to original
        assert_eq!(
            fs::read(target_dir.path().join("a.bin")).unwrap(),
            b"original a"
        );
    }

    #[test]
    fn backup_preserved_on_success() {
        let orig_dir = tempdir().unwrap();
        let new_dir = tempdir().unwrap();
        let patch_dir = tempdir().unwrap();
        let target_dir = tempdir().unwrap();

        fs::write(orig_dir.path().join("file.bin"), b"original").unwrap();
        fs::write(new_dir.path().join("file.bin"), b"modified").unwrap();
        patch_create::run(orig_dir.path(), new_dir.path(), patch_dir.path(), 1, None, true).unwrap();

        fs::write(target_dir.path().join("file.bin"), b"original").unwrap();

        run(target_dir.path(), patch_dir.path()).unwrap();

        // Backup directory should exist with original file
        let backup_dir = target_dir.path().join(BACKUP_DIR);
        assert!(backup_dir.exists());
        assert_eq!(fs::read(backup_dir.join("file.bin")).unwrap(), b"original");
    }

    #[test]
    fn missing_manifest_returns_error() {
        let target_dir = tempdir().unwrap();
        let patch_dir = tempdir().unwrap();

        let result = run(target_dir.path(), patch_dir.path());

        assert!(matches!(result, Err(PatchError::ManifestError { .. })));
    }
}
