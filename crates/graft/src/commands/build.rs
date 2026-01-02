//! Create self-appending patcher executables.
//!
//! This command creates standalone patcher binaries by concatenating
//! a pre-built stub with the patch archive data.
//!
//! For macOS targets, modifies stub .app bundles with patch data and custom metadata.

use crate::commands::macos_bundle::{self, BundleError};
use crate::commands::windows_icon::{self, WindowsIconError};
use crate::stubs::{self, StubError};
use crate::targets::{self, Target};
#[cfg(feature = "embedded-stubs")]
use crate::targets::ALL_TARGETS;
use graft_core::archive::{self, MAGIC_MARKER};
use graft_core::patch::{self, ASSETS_DIR, ICON_FILENAME};
use graft_core::utils::manifest::PatchInfo;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Source for stub binaries.
enum StubSource<'a> {
    /// Use stubs from a directory.
    Directory(&'a Path),
    /// Use embedded stubs (production mode only).
    #[cfg(feature = "embedded-stubs")]
    Embedded,
}

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

/// Output filename for a target.
fn output_filename(target: &Target) -> String {
    if target.stub_is_bundle {
        format!("patcher-{}.app", target.name)
    } else {
        format!("patcher-{}{}", target.name, target.binary_suffix)
    }
}

/// Resolve target list. If empty, returns all available targets for the stub source.
fn resolve_targets(
    stub_source: &StubSource<'_>,
    target_names: &[String],
) -> Result<Vec<Target>, PatcherError> {
    if target_names.is_empty() {
        // Default to all available targets
        let available: Vec<Target> = match stub_source {
            StubSource::Directory(dir) => {
                stubs::find_available_targets_in_dir(dir)
                    .into_iter()
                    .copied()
                    .collect()
            }
            #[cfg(feature = "embedded-stubs")]
            StubSource::Embedded => ALL_TARGETS.to_vec(),
        };
        if available.is_empty() {
            return Err(PatcherError::InvalidTarget(
                "No stubs available for any target".to_string(),
            ));
        }
        Ok(available)
    } else {
        // Parse specified targets
        target_names
            .iter()
            .map(|name| {
                targets::parse_target(name)
                    .ok_or_else(|| PatcherError::InvalidTarget(name.clone()))
            })
            .collect()
    }
}

/// Create a patcher executable (production mode with embedded stubs).
///
/// # Arguments
/// * `patch_dir` - Path to the patch directory (containing manifest.json)
/// * `output_dir` - Output directory for patcher executables
/// * `stub_dir` - Optional directory with stubs (overrides embedded)
/// * `targets` - Target platforms to build for (empty = all available)
#[cfg(feature = "embedded-stubs")]
pub fn run(
    patch_dir: &Path,
    output_dir: &Path,
    stub_dir: Option<&Path>,
    targets: &[String],
) -> Result<(), PatcherError> {
    let stub_source = match stub_dir {
        Some(dir) => StubSource::Directory(dir),
        None => StubSource::Embedded,
    };

    let targets_to_build = resolve_targets(&stub_source, targets)?;

    // Ensure output directory exists
    fs::create_dir_all(output_dir).map_err(PatcherError::OutputError)?;

    for target in &targets_to_build {
        build_single(patch_dir, target, output_dir, &stub_source)?;
    }

    Ok(())
}

/// Create a patcher executable (development mode without embedded stubs).
///
/// # Arguments
/// * `patch_dir` - Path to the patch directory (containing manifest.json)
/// * `output_dir` - Output directory for patcher executables
/// * `stub_dir` - Directory containing stub binaries (required)
/// * `targets` - Target platforms to build for (empty = all available)
#[cfg(not(feature = "embedded-stubs"))]
pub fn run(
    patch_dir: &Path,
    output_dir: &Path,
    stub_dir: &Path,
    targets: &[String],
) -> Result<(), PatcherError> {
    println!("Development mode: no embedded stubs");
    println!("Using stubs from: {}", stub_dir.display());
    println!();

    let stub_source = StubSource::Directory(stub_dir);
    let targets_to_build = resolve_targets(&stub_source, targets)?;

    // Ensure output directory exists
    fs::create_dir_all(output_dir).map_err(PatcherError::OutputError)?;

    for target in &targets_to_build {
        build_single(patch_dir, target, output_dir, &stub_source)?;
    }

    Ok(())
}

/// Build a patcher for a single target.
fn build_single(
    patch_dir: &Path,
    target: &Target,
    output_dir: &Path,
    stub_source: &StubSource<'_>,
) -> Result<(), PatcherError> {
    // Validate patch directory
    let manifest = patch::validate_patch_dir(patch_dir)
        .map_err(|e| PatcherError::PatchValidation(e.to_string()))?;
    let info = PatchInfo::from_manifest(&manifest);

    println!(
        "Creating patcher for patch v{} ({} operations: {} patches, {} additions, {} deletions)",
        info.version, info.entry_count, info.patches, info.additions, info.deletions
    );
    println!("Target: {}", target.name);

    // Create archive
    print!("Creating patch archive... ");
    io::stdout().flush().ok();
    let archive_data =
        archive::create_archive_bytes(patch_dir).map_err(PatcherError::ArchiveCreation)?;
    println!("done ({} bytes)", archive_data.len());

    // Determine output path
    let output = output_dir.join(output_filename(target));

    // Build patcher based on target type
    if target.stub_is_bundle {
        // macOS: Get stub bundle, copy and modify it
        print!("Getting stub bundle... ");
        io::stdout().flush().ok();
        let stub_bundle_path = get_stub_bundle(target, stub_source)?;
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
        let stub_data = get_stub(target, stub_source)?;
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

/// Get stub binary from the appropriate source.
fn get_stub(target: &Target, stub_source: &StubSource<'_>) -> Result<Vec<u8>, PatcherError> {
    match stub_source {
        StubSource::Directory(dir) => {
            stubs::read_stub_from_dir(dir, target).map_err(PatcherError::StubError)
        }
        #[cfg(feature = "embedded-stubs")]
        StubSource::Embedded => stubs::get_embedded_stub(target).map_err(PatcherError::StubError),
    }
}

/// Get stub bundle path from the appropriate source.
fn get_stub_bundle(target: &Target, stub_source: &StubSource<'_>) -> Result<PathBuf, PatcherError> {
    match stub_source {
        StubSource::Directory(dir) => {
            stubs::read_stub_bundle_from_dir(dir, target).map_err(PatcherError::StubError)
        }
        #[cfg(feature = "embedded-stubs")]
        StubSource::Embedded => {
            stubs::get_embedded_stub_bundle(target).map_err(PatcherError::StubError)
        }
    }
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
        let output_dir = temp.path().join("output");
        let stub_dir = temp.path().join("stubs");
        fs::create_dir_all(&stub_dir).unwrap();

        // Specify a target to bypass stub availability check
        let targets = vec!["linux-x64".to_string()];

        #[cfg(feature = "embedded-stubs")]
        let result = run(temp.path(), &output_dir, Some(&stub_dir), &targets);

        #[cfg(not(feature = "embedded-stubs"))]
        let result = run(temp.path(), &output_dir, &stub_dir, &targets);

        assert!(matches!(result, Err(PatcherError::PatchValidation(_))));
    }

    #[test]
    fn run_fails_with_invalid_target() {
        let temp = tempdir().unwrap();
        let output_dir = temp.path().join("output");
        let stub_dir = temp.path().join("stubs");
        fs::create_dir_all(&stub_dir).unwrap();

        // Create minimal patch structure
        fs::write(
            temp.path().join("manifest.json"),
            r#"{"version": 1, "entries": []}"#,
        )
        .unwrap();

        let targets = vec!["invalid-target".to_string()];

        #[cfg(feature = "embedded-stubs")]
        let result = run(temp.path(), &output_dir, Some(&stub_dir), &targets);

        #[cfg(not(feature = "embedded-stubs"))]
        let result = run(temp.path(), &output_dir, &stub_dir, &targets);

        assert!(matches!(result, Err(PatcherError::InvalidTarget(_))));
    }
}
