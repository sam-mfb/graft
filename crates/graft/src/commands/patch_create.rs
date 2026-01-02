use std::fs;
use std::io;
use std::path::Path;

use graft_core::patch::{ASSETS_DIR, DIFFS_DIR, DIFF_EXTENSION, FILES_DIR, ICON_FILENAME, MANIFEST_FILENAME};
use graft_core::utils::diff::create_diff;
use graft_core::utils::dir_scan::{categorize_files, FileChange};
use graft_core::utils::hash::hash_bytes;
use graft_core::utils::manifest::{Manifest, ManifestEntry};

/// Default icon embedded at compile time
const DEFAULT_ICON: &[u8] = include_bytes!("../../assets/default_icon.png");

/// Create a patch from two directories.
/// Outputs a patch directory containing manifest.json, diffs/, and files/.
///
/// If `allow_restricted` is true, the resulting manifest will allow patching
/// restricted paths (system directories, executables). Default is false for security.
pub fn run(
    orig_dir: &Path,
    new_dir: &Path,
    output_dir: &Path,
    version: u32,
    title: Option<&str>,
    allow_restricted: bool,
) -> io::Result<()> {
    let changes = categorize_files(orig_dir, new_dir)?;

    // Create output directory structure
    fs::create_dir_all(output_dir)?;
    let diffs_dir = output_dir.join(DIFFS_DIR);
    let files_dir = output_dir.join(FILES_DIR);

    // Only create subdirs if we need them
    let has_diffs = changes.iter().any(|c| matches!(c, FileChange::Diff { .. }));
    let has_new = changes.iter().any(|c| matches!(c, FileChange::New { .. }));

    if has_diffs {
        fs::create_dir_all(&diffs_dir)?;
    }
    if has_new {
        fs::create_dir_all(&files_dir)?;
    }

    let mut manifest = Manifest::new(version, title.map(|s| s.to_string()));
    manifest.allow_restricted = allow_restricted;

    for change in changes {
        let entry = match change {
            FileChange::Diff {
                file,
                original_hash,
                final_hash,
            } => {
                // Read files and create diff
                let orig_data = fs::read(orig_dir.join(&file))?;
                let new_data = fs::read(new_dir.join(&file))?;
                let diff_data = create_diff(&orig_data, &new_data)?;

                // Write diff file
                let diff_path = diffs_dir.join(format!("{}{}", file, DIFF_EXTENSION));
                fs::write(&diff_path, &diff_data)?;

                // Compute diff hash
                let diff_hash = hash_bytes(&diff_data);

                ManifestEntry::Patch {
                    file,
                    original_hash,
                    diff_hash,
                    final_hash,
                }
            }
            FileChange::New { file, final_hash } => {
                // Copy new file to files/
                let src_path = new_dir.join(&file);
                let dest_path = files_dir.join(&file);
                fs::copy(&src_path, &dest_path)?;

                ManifestEntry::Add { file, final_hash }
            }
            FileChange::Old {
                file,
                original_hash,
            } => {
                // Nothing to write, just record in manifest
                ManifestEntry::Delete { file, original_hash }
            }
        };

        manifest.entries.push(entry);
    }

    // Sort entries by filename for consistent output
    manifest.entries.sort_by(|a, b| a.file().cmp(b.file()));

    // Write manifest
    let manifest_path = output_dir.join(MANIFEST_FILENAME);
    manifest.save(&manifest_path)?;

    // Create assets directory with default icon
    let assets_dir = output_dir.join(ASSETS_DIR);
    fs::create_dir_all(&assets_dir)?;
    fs::write(assets_dir.join(ICON_FILENAME), DEFAULT_ICON)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use graft_core::utils::diff::apply_diff;
    use tempfile::tempdir;

    #[test]
    fn creates_directory_structure() {
        let orig_dir = tempdir().unwrap();
        let new_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();

        // Create a modified file (triggers diffs/ creation)
        fs::write(orig_dir.path().join("modified.bin"), b"old").unwrap();
        fs::write(new_dir.path().join("modified.bin"), b"new").unwrap();

        // Create a new file (triggers files/ creation)
        fs::write(new_dir.path().join("added.bin"), b"added").unwrap();

        run(orig_dir.path(), new_dir.path(), output_dir.path(), 1, None, false).unwrap();

        assert!(output_dir.path().join("manifest.json").exists());
        assert!(output_dir.path().join("diffs").exists());
        assert!(output_dir.path().join("files").exists());
    }

    #[test]
    fn creates_valid_diffs() {
        let orig_dir = tempdir().unwrap();
        let new_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();

        let orig_content = b"original content here";
        let new_content = b"modified content here";

        fs::write(orig_dir.path().join("file.bin"), orig_content).unwrap();
        fs::write(new_dir.path().join("file.bin"), new_content).unwrap();

        run(orig_dir.path(), new_dir.path(), output_dir.path(), 1, None, false).unwrap();

        // Read the diff and apply it
        let diff_data = fs::read(output_dir.path().join("diffs").join("file.bin.diff")).unwrap();
        let result = apply_diff(orig_content, &diff_data).unwrap();

        assert_eq!(result, new_content);
    }

    #[test]
    fn copies_new_files() {
        let orig_dir = tempdir().unwrap();
        let new_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();

        let content = b"new file content";
        fs::write(new_dir.path().join("new.bin"), content).unwrap();

        run(orig_dir.path(), new_dir.path(), output_dir.path(), 1, None, false).unwrap();

        let copied = fs::read(output_dir.path().join("files").join("new.bin")).unwrap();
        assert_eq!(copied, content);
    }

    #[test]
    fn manifest_contains_correct_entries() {
        let orig_dir = tempdir().unwrap();
        let new_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();

        // Modified file
        fs::write(orig_dir.path().join("modified.bin"), b"old").unwrap();
        fs::write(new_dir.path().join("modified.bin"), b"new").unwrap();

        // New file
        fs::write(new_dir.path().join("added.bin"), b"added").unwrap();

        // Deleted file
        fs::write(orig_dir.path().join("deleted.bin"), b"deleted").unwrap();

        // Unchanged file (should not appear in manifest)
        fs::write(orig_dir.path().join("unchanged.bin"), b"same").unwrap();
        fs::write(new_dir.path().join("unchanged.bin"), b"same").unwrap();

        run(orig_dir.path(), new_dir.path(), output_dir.path(), 1, None, false).unwrap();

        let manifest = Manifest::load(&output_dir.path().join("manifest.json")).unwrap();

        assert_eq!(manifest.entries.len(), 3);

        // Check each entry type exists
        assert!(manifest
            .entries
            .iter()
            .any(|e| matches!(e, ManifestEntry::Patch { file, .. } if file == "modified.bin")));
        assert!(manifest
            .entries
            .iter()
            .any(|e| matches!(e, ManifestEntry::Add { file, .. } if file == "added.bin")));
        assert!(manifest
            .entries
            .iter()
            .any(|e| matches!(e, ManifestEntry::Delete { file, .. } if file == "deleted.bin")));
    }

    #[test]
    fn manifest_has_correct_hashes() {
        let orig_dir = tempdir().unwrap();
        let new_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();

        let orig_content = b"original";
        let new_content = b"modified";

        fs::write(orig_dir.path().join("file.bin"), orig_content).unwrap();
        fs::write(new_dir.path().join("file.bin"), new_content).unwrap();

        run(orig_dir.path(), new_dir.path(), output_dir.path(), 1, None, false).unwrap();

        let manifest = Manifest::load(&output_dir.path().join("manifest.json")).unwrap();

        if let ManifestEntry::Patch {
            original_hash,
            diff_hash,
            final_hash,
            ..
        } = &manifest.entries[0]
        {
            assert_eq!(original_hash, &hash_bytes(orig_content));
            assert_eq!(final_hash, &hash_bytes(new_content));

            // Verify diff_hash matches the actual diff file
            let diff_data =
                fs::read(output_dir.path().join("diffs").join("file.bin.diff")).unwrap();
            assert_eq!(diff_hash, &hash_bytes(&diff_data));
        } else {
            panic!("Expected Patch entry");
        }
    }

    #[test]
    fn empty_directories_creates_empty_manifest() {
        let orig_dir = tempdir().unwrap();
        let new_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();

        run(orig_dir.path(), new_dir.path(), output_dir.path(), 1, None, false).unwrap();

        let manifest = Manifest::load(&output_dir.path().join("manifest.json")).unwrap();
        assert!(manifest.entries.is_empty());
    }

    #[test]
    fn skips_unnecessary_subdirs() {
        let orig_dir = tempdir().unwrap();
        let new_dir = tempdir().unwrap();
        let output_dir = tempdir().unwrap();

        // Only a deleted file - no diffs/ or files/ needed
        fs::write(orig_dir.path().join("deleted.bin"), b"deleted").unwrap();

        run(orig_dir.path(), new_dir.path(), output_dir.path(), 1, None, false).unwrap();

        assert!(output_dir.path().join("manifest.json").exists());
        assert!(!output_dir.path().join("diffs").exists());
        assert!(!output_dir.path().join("files").exists());
    }
}
