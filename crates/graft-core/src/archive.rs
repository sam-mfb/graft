//! Archive creation and constants for self-appending binaries.
//!
//! This module handles creating tar.gz archives from patch directories
//! and defines the magic marker used for self-appending binary detection.

use crate::patch;
use flate2::write::GzEncoder;
use flate2::Compression;
use std::fs;
use std::io::{self, Write};
use std::path::Path;
use tar::Builder;

/// Magic marker at end of self-appending binary.
/// Used to detect if a binary has patch data appended.
pub const MAGIC_MARKER: &[u8; 8] = b"GRAFTPCH";

/// Create a tar.gz archive from a patch directory.
///
/// The archive will contain:
/// - manifest.json (required)
/// - diffs/*.diff (if present)
/// - files/* (if present)
///
/// Returns the compressed bytes.
pub fn create_archive_bytes(patch_dir: &Path) -> io::Result<Vec<u8>> {
    let mut buffer = Vec::new();

    {
        let encoder = GzEncoder::new(&mut buffer, Compression::default());
        let mut archive = Builder::new(encoder);

        // Add manifest.json (required)
        let manifest_path = patch_dir.join(patch::MANIFEST_FILENAME);
        archive.append_path_with_name(&manifest_path, patch::MANIFEST_FILENAME)?;

        // Add diffs directory if it exists
        let diffs_path = patch_dir.join(patch::DIFFS_DIR);
        if diffs_path.is_dir() {
            add_directory_contents(&mut archive, &diffs_path, patch::DIFFS_DIR)?;
        }

        // Add files directory if it exists
        let files_path = patch_dir.join(patch::FILES_DIR);
        if files_path.is_dir() {
            add_directory_contents(&mut archive, &files_path, patch::FILES_DIR)?;
        }

        // Finish the archive
        let encoder = archive.into_inner()?;
        encoder.finish()?;
    }

    Ok(buffer)
}

/// Recursively add directory contents to the archive.
fn add_directory_contents<W: Write>(
    archive: &mut Builder<W>,
    dir: &Path,
    archive_prefix: &str,
) -> io::Result<()> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let archive_path = format!("{}/{}", archive_prefix, file_name.to_string_lossy());

        if path.is_file() {
            archive.append_path_with_name(&path, &archive_path)?;
        } else if path.is_dir() {
            // Recursively add subdirectories (for nested file structures in files/)
            add_directory_contents(archive, &path, &archive_path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::read::GzDecoder;
    use tar::Archive;
    use tempfile::tempdir;

    #[test]
    fn creates_valid_archive() {
        let patch_dir = tempdir().unwrap();

        // Create minimal patch structure
        fs::write(
            patch_dir.path().join("manifest.json"),
            r#"{"version": 1, "entries": []}"#,
        )
        .unwrap();

        let archive_data = create_archive_bytes(patch_dir.path()).unwrap();

        // Verify it's valid gzip + tar
        let decoder = GzDecoder::new(&archive_data[..]);
        let mut archive = Archive::new(decoder);

        let entries: Vec<_> = archive.entries().unwrap().collect();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn includes_diffs_directory() {
        let patch_dir = tempdir().unwrap();

        fs::write(
            patch_dir.path().join("manifest.json"),
            r#"{"version": 1, "entries": []}"#,
        )
        .unwrap();

        fs::create_dir(patch_dir.path().join("diffs")).unwrap();
        fs::write(patch_dir.path().join("diffs/file.diff"), b"diff data").unwrap();

        let archive_data = create_archive_bytes(patch_dir.path()).unwrap();

        let decoder = GzDecoder::new(&archive_data[..]);
        let mut archive = Archive::new(decoder);

        let paths: Vec<_> = archive
            .entries()
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path().unwrap().to_path_buf())
            .collect();

        assert!(paths
            .iter()
            .any(|p| p.to_string_lossy().contains("diffs/file.diff")));
    }

    #[test]
    fn includes_files_directory() {
        let patch_dir = tempdir().unwrap();

        fs::write(
            patch_dir.path().join("manifest.json"),
            r#"{"version": 1, "entries": []}"#,
        )
        .unwrap();

        fs::create_dir(patch_dir.path().join("files")).unwrap();
        fs::write(
            patch_dir.path().join("files/new_file.bin"),
            b"new file data",
        )
        .unwrap();

        let archive_data = create_archive_bytes(patch_dir.path()).unwrap();

        let decoder = GzDecoder::new(&archive_data[..]);
        let mut archive = Archive::new(decoder);

        let paths: Vec<_> = archive
            .entries()
            .unwrap()
            .filter_map(|e| e.ok())
            .map(|e| e.path().unwrap().to_path_buf())
            .collect();

        assert!(paths
            .iter()
            .any(|p| p.to_string_lossy().contains("files/new_file.bin")));
    }

    #[test]
    fn magic_marker_is_correct() {
        assert_eq!(MAGIC_MARKER, b"GRAFTPCH");
        assert_eq!(MAGIC_MARKER.len(), 8);
    }
}
