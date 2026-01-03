//! Stub binary provider - handles embedded and directory-based stubs.
//!
//! This module provides stub binaries for patcher creation. Stubs can come from:
//! 1. Embedded in the binary (when compiled with `embedded-stubs` feature)
//! 2. A directory specified by the user (for development or custom stubs)
//!
//! For macOS targets, stubs are distributed as .app bundles (zipped). For embedded
//! stubs, use `extract_embedded_stub_bundle_to()` to extract directly to the output
//! location. For directory-based stubs, use `read_stub_bundle_from_dir()`.

use crate::targets::{self, Target, ALL_TARGETS};
use std::fs::{self, File};
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};

/// Errors that can occur when getting stubs.
#[derive(Debug)]
pub enum StubError {
    /// The requested target is not available.
    TargetNotAvailable(String),
    /// Failed to read stub from file.
    ReadFailed { path: PathBuf, source: io::Error },
    /// Failed to extract bundle.
    ExtractFailed(String),
    /// Temporary directory error.
    TempDirError(io::Error),
}

impl std::fmt::Display for StubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StubError::TargetNotAvailable(t) => write!(f, "Stub not available for target: {}", t),
            StubError::ReadFailed { path, source } => {
                write!(f, "Failed to read stub {}: {}", path.display(), source)
            }
            StubError::ExtractFailed(msg) => write!(f, "Failed to extract bundle: {}", msg),
            StubError::TempDirError(e) => write!(f, "Temporary directory error: {}", e),
        }
    }
}

impl std::error::Error for StubError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StubError::ReadFailed { source, .. } => Some(source),
            StubError::TempDirError(e) => Some(e),
            _ => None,
        }
    }
}

/// Read a stub binary from a directory.
pub fn read_stub_from_dir(dir: &Path, target: &Target) -> Result<Vec<u8>, StubError> {
    let path = dir.join(targets::stub_filename(target));
    fs::read(&path).map_err(|source| StubError::ReadFailed { path, source })
}

/// Read a stub bundle from a directory.
///
/// Handles both:
/// - `.app.zip` files (extracts to temp directory)
/// - Direct `.app` directories
pub fn read_stub_bundle_from_dir(dir: &Path, target: &Target) -> Result<PathBuf, StubError> {
    // Try .app.zip first
    let zip_path = dir.join(targets::stub_filename(target));
    if zip_path.exists() {
        // Extract to temp directory
        let zip_data = fs::read(&zip_path).map_err(|source| StubError::ReadFailed {
            path: zip_path.clone(),
            source,
        })?;

        let temp_dir = std::env::temp_dir().join("graft-stubs");
        fs::create_dir_all(&temp_dir).map_err(StubError::TempDirError)?;

        let bundle_name = format!("graft-gui-stub-{}.app", target.name);
        let bundle_path = temp_dir.join(&bundle_name);

        // Extract if not already present
        if !bundle_path.exists() {
            extract_zip(&zip_data, &bundle_path)?;
        }

        return Ok(bundle_path);
    }

    // Try direct .app directory
    let app_name = format!("graft-gui-stub-{}.app", target.name);
    let app_path = dir.join(&app_name);
    if app_path.exists() && app_path.is_dir() {
        return Ok(app_path);
    }

    Err(StubError::TargetNotAvailable(format!(
        "No stub found for {} in {}",
        target.name,
        dir.display()
    )))
}

/// Find all available targets in a stub directory.
pub fn find_available_targets_in_dir(dir: &Path) -> Vec<&'static Target> {
    ALL_TARGETS
        .iter()
        .filter(|t| {
            let stub_path = dir.join(targets::stub_filename(t));
            if stub_path.exists() {
                return true;
            }
            // Also check for direct .app directories
            if t.stub_is_bundle {
                let app_name = format!("graft-gui-stub-{}.app", t.name);
                let app_path = dir.join(&app_name);
                if app_path.exists() && app_path.is_dir() {
                    return true;
                }
            }
            false
        })
        .collect()
}

/// Extract a zip archive to the specified directory.
fn extract_zip(zip_data: &[u8], output_path: &Path) -> Result<(), StubError> {
    let reader = Cursor::new(zip_data);
    let mut archive = zip::ZipArchive::new(reader)
        .map_err(|e| StubError::ExtractFailed(format!("Invalid zip: {}", e)))?;

    // The zip contains a single .app directory at the root
    // We need to extract it to output_path, renaming the root directory

    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| StubError::ExtractFailed(format!("Failed to read entry: {}", e)))?;

        let entry_path = file
            .enclosed_name()
            .ok_or_else(|| StubError::ExtractFailed("Invalid path in zip".to_string()))?;

        // Get path components
        let components: Vec<_> = entry_path.components().collect();
        if components.is_empty() {
            continue;
        }

        // Skip the root .app directory name and reconstruct path under output_path
        let relative_path: PathBuf = if components.len() > 1 {
            components[1..].iter().collect()
        } else {
            // This is the root .app directory itself
            PathBuf::new()
        };

        let target_path = output_path.join(&relative_path);

        if file.is_dir() {
            fs::create_dir_all(&target_path)
                .map_err(|e| StubError::ExtractFailed(format!("Failed to create dir: {}", e)))?;
        } else {
            // Ensure parent directory exists
            if let Some(parent) = target_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| StubError::ExtractFailed(format!("Failed to create dir: {}", e)))?;
            }

            let mut outfile = File::create(&target_path)
                .map_err(|e| StubError::ExtractFailed(format!("Failed to create file: {}", e)))?;

            io::copy(&mut file, &mut outfile)
                .map_err(|e| StubError::ExtractFailed(format!("Failed to write file: {}", e)))?;

            // Preserve executable permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Some(mode) = file.unix_mode() {
                    let permissions = std::fs::Permissions::from_mode(mode);
                    fs::set_permissions(&target_path, permissions).ok();
                }
            }
        }
    }

    Ok(())
}

// ============================================================================
// Embedded stubs (when compiled with embedded-stubs feature)
// ============================================================================

/// Get embedded stub binary for non-bundle targets.
#[cfg(feature = "embedded-stubs")]
pub fn get_embedded_stub(target: &Target) -> Result<Vec<u8>, StubError> {
    match target.name {
        "linux-x64" => Ok(include_bytes!(concat!(
            env!("GRAFT_STUBS_DIR"),
            "/graft-gui-stub-linux-x64"
        ))
        .to_vec()),
        "linux-arm64" => Ok(include_bytes!(concat!(
            env!("GRAFT_STUBS_DIR"),
            "/graft-gui-stub-linux-arm64"
        ))
        .to_vec()),
        "windows-x64" => Ok(include_bytes!(concat!(
            env!("GRAFT_STUBS_DIR"),
            "/graft-gui-stub-windows-x64.exe"
        ))
        .to_vec()),
        // macOS uses bundle stubs, not binary stubs
        _ => Err(StubError::TargetNotAvailable(target.name.to_string())),
    }
}

/// Extract embedded stub bundle directly to the specified output path.
///
/// This extracts the embedded zip directly to the output location without
/// using any temporary files or caching, which is more secure.
#[cfg(feature = "embedded-stubs")]
pub fn extract_embedded_stub_bundle_to(target: &Target, output_path: &Path) -> Result<(), StubError> {
    let zip_data: &[u8] = match target.name {
        "macos-x64" => include_bytes!(concat!(
            env!("GRAFT_STUBS_DIR"),
            "/graft-gui-stub-macos-x64.app.zip"
        )),
        "macos-arm64" => include_bytes!(concat!(
            env!("GRAFT_STUBS_DIR"),
            "/graft-gui-stub-macos-arm64.app.zip"
        )),
        _ => return Err(StubError::TargetNotAvailable(target.name.to_string())),
    };

    extract_zip(zip_data, output_path)
}
