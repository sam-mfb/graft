//! Create self-appending patcher executables.
//!
//! This command creates standalone patcher binaries by concatenating
//! a pre-built stub with the patch archive data.
//!
//! For macOS targets, modifies stub .app bundles with patch data and custom metadata.

use crate::commands::macos_bundle::{self, BundleError};
use crate::commands::windows_icon::{self, WindowsIconError};
use crate::stubs::{self, StubError};
use crate::targets;
use graft_core::archive::{self, MAGIC_MARKER};
use graft_core::patch::{self, ASSETS_DIR, ICON_FILENAME};
use graft_core::utils::manifest::PatchInfo;
use std::fs;
use std::io::{self, Write};
use std::path::Path;

/// Errors from patcher creation.
#[derive(Debug)]
pub enum PatcherError {
    /// Patch directory validation failed.
    PatchValidation(String),
    /// Failed to create the patch archive.
    ArchiveCreation(io::Error),
    /// Failed to get the stub binary.
    StubError(StubError),
    /// Failed to write the output file.
    OutputError(io::Error),
    /// Invalid target specified.
    InvalidTarget(String),
    /// Failed to create macOS bundle.
    BundleError(BundleError),
    /// Failed to embed Windows icon.
    WindowsIconError(WindowsIconError),
}

impl std::fmt::Display for PatcherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatcherError::PatchValidation(msg) => write!(f, "Patch validation failed: {}", msg),
            PatcherError::ArchiveCreation(e) => write!(f, "Failed to create archive: {}", e),
            PatcherError::StubError(e) => write!(f, "Stub error: {}", e),
            PatcherError::OutputError(e) => write!(f, "Output error: {}", e),
            PatcherError::InvalidTarget(t) => write!(f, "Invalid target: {}", t),
            PatcherError::BundleError(e) => write!(f, "Bundle creation failed: {}", e),
            PatcherError::WindowsIconError(e) => write!(f, "Windows icon embedding failed: {}", e),
        }
    }
}

impl std::error::Error for PatcherError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PatcherError::ArchiveCreation(e) => Some(e),
            PatcherError::StubError(e) => Some(e),
            PatcherError::OutputError(e) => Some(e),
            PatcherError::BundleError(e) => Some(e),
            PatcherError::WindowsIconError(e) => Some(e),
            _ => None,
        }
    }
}

/// Create a self-appending patcher executable.
///
/// # Arguments
/// * `patch_dir` - Path to the patch directory (containing manifest.json)
/// * `target_name` - Optional target platform name (defaults to current platform)
/// * `output_path` - Optional output file path (defaults to ./patcher, ./patcher.exe, or ./patcher.app)
pub fn run(
    patch_dir: &Path,
    target_name: Option<&str>,
    output_path: Option<&Path>,
) -> Result<(), PatcherError> {
    // 1. Resolve target
    let target = match target_name {
        Some(name) => targets::parse_target(name)
            .ok_or_else(|| PatcherError::InvalidTarget(name.to_string()))?,
        None => targets::current_target()
            .ok_or_else(|| PatcherError::InvalidTarget("current platform not supported".to_string()))?,
    };

    // 2. Validate patch directory
    let manifest = patch::validate_patch_dir(patch_dir)
        .map_err(|e| PatcherError::PatchValidation(e.to_string()))?;
    let info = PatchInfo::from_manifest(&manifest);

    println!(
        "Creating patcher for patch v{} ({} operations: {} patches, {} additions, {} deletions)",
        info.version, info.entry_count, info.patches, info.additions, info.deletions
    );
    println!("Target: {}", target.name);

    // 3. Create archive
    print!("Creating patch archive... ");
    io::stdout().flush().ok();
    let archive_data =
        archive::create_archive_bytes(patch_dir).map_err(PatcherError::ArchiveCreation)?;
    println!("done ({} bytes)", archive_data.len());

    // 4. Determine output path
    let output = match output_path {
        Some(p) => p.to_path_buf(),
        None => {
            if target.stub_is_bundle {
                Path::new(".").join("patcher.app")
            } else {
                let name = format!("patcher{}", target.binary_suffix);
                Path::new(".").join(name)
            }
        }
    };

    // 5. Build patcher based on target type
    if target.stub_is_bundle {
        // macOS: Get stub bundle, copy and modify it
        print!("Getting stub bundle... ");
        io::stdout().flush().ok();
        let stub_bundle_path = stubs::get_stub_bundle(&target).map_err(PatcherError::StubError)?;
        println!("done");

        print!("Creating macOS bundle at {}... ", output.display());
        io::stdout().flush().ok();

        let total_size = macos_bundle::modify_bundle(
            &stub_bundle_path,
            &output,
            &archive_data,
            patch_dir,
            info.title.as_deref(),
            &info.version.to_string(),
        )
        .map_err(PatcherError::BundleError)?;

        println!("done");
        println!();
        println!("Created: {} ({} bytes executable)", output.display(), total_size);
    } else {
        // Other platforms: Get stub binary, concatenate with archive
        print!("Getting stub binary... ");
        io::stdout().flush().ok();
        let stub_data = stubs::get_stub(&target).map_err(PatcherError::StubError)?;
        println!("done ({} bytes)", stub_data.len());

        let executable_data = create_executable_bytes(&stub_data, &archive_data);
        let total_size = executable_data.len();

        print!("Writing patcher to {}... ", output.display());
        io::stdout().flush().ok();

        fs::write(&output, &executable_data).map_err(PatcherError::OutputError)?;
        println!("done");

        // Embed icon for Windows targets
        if target.name.starts_with("windows-") {
            let icon_path = patch_dir.join(ASSETS_DIR).join(ICON_FILENAME);
            if icon_path.exists() {
                print!("Embedding icon... ");
                io::stdout().flush().ok();
                windows_icon::embed_icon(&output, &icon_path)
                    .map_err(PatcherError::WindowsIconError)?;
                println!("done");
            }
        }

        // Make executable on Unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = fs::metadata(&output)
                .map_err(PatcherError::OutputError)?
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&output, perms).map_err(PatcherError::OutputError)?;
        }

        println!();
        println!("Created: {} ({} bytes)", output.display(), total_size);
    }

    Ok(())
}

/// Create the combined executable bytes (stub + archive + size + magic).
fn create_executable_bytes(stub_data: &[u8], archive_data: &[u8]) -> Vec<u8> {
    let mut data = Vec::with_capacity(stub_data.len() + archive_data.len() + 16);

    // Write stub
    data.extend_from_slice(stub_data);

    // Write archive
    data.extend_from_slice(archive_data);

    // Write size (8 bytes, little-endian)
    let size_bytes = (archive_data.len() as u64).to_le_bytes();
    data.extend_from_slice(&size_bytes);

    // Write magic marker
    data.extend_from_slice(MAGIC_MARKER);

    data
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn run_fails_with_invalid_patch_dir() {
        let temp = tempdir().unwrap();
        let result = run(temp.path(), None, None);
        assert!(matches!(result, Err(PatcherError::PatchValidation(_))));
    }

    #[test]
    fn run_fails_with_invalid_target() {
        let temp = tempdir().unwrap();
        // Create minimal patch structure
        fs::write(
            temp.path().join("manifest.json"),
            r#"{"version": 1, "entries": []}"#,
        )
        .unwrap();

        let result = run(temp.path(), Some("invalid-target"), None);
        assert!(matches!(result, Err(PatcherError::InvalidTarget(_))));
    }
}
