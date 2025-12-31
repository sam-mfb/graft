//! Create self-appending patcher executables.
//!
//! This command creates standalone patcher binaries by concatenating
//! a pre-built stub with the patch archive data.

use crate::stubs::{self, StubError};
use crate::targets;
use graft_core::archive::{self, MAGIC_MARKER};
use graft_core::patch;
use graft_core::utils::manifest::PatchInfo;
use std::fs::{self, File};
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
}

impl std::fmt::Display for PatcherError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PatcherError::PatchValidation(msg) => write!(f, "Patch validation failed: {}", msg),
            PatcherError::ArchiveCreation(e) => write!(f, "Failed to create archive: {}", e),
            PatcherError::StubError(e) => write!(f, "Stub error: {}", e),
            PatcherError::OutputError(e) => write!(f, "Output error: {}", e),
            PatcherError::InvalidTarget(t) => write!(f, "Invalid target: {}", t),
        }
    }
}

impl std::error::Error for PatcherError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            PatcherError::ArchiveCreation(e) => Some(e),
            PatcherError::StubError(e) => Some(e),
            PatcherError::OutputError(e) => Some(e),
            _ => None,
        }
    }
}

/// Create a self-appending patcher executable.
///
/// # Arguments
/// * `patch_dir` - Path to the patch directory (containing manifest.json)
/// * `target_name` - Optional target platform name (defaults to current platform)
/// * `output_path` - Optional output file path (defaults to ./patcher or ./patcher.exe)
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

    // 4. Get stub
    print!("Getting stub binary... ");
    io::stdout().flush().ok();
    let stub_data = stubs::get_stub(&target).map_err(PatcherError::StubError)?;
    println!("done ({} bytes)", stub_data.len());

    // 5. Determine output path
    let output = match output_path {
        Some(p) => p.to_path_buf(),
        None => {
            let name = format!("patcher{}", target.binary_suffix);
            Path::new(".").join(name)
        }
    };

    // 6. Create output file with appended data
    print!("Writing patcher to {}... ", output.display());
    io::stdout().flush().ok();

    let mut file = File::create(&output).map_err(PatcherError::OutputError)?;

    // Write stub
    file.write_all(&stub_data)
        .map_err(PatcherError::OutputError)?;

    // Write archive
    file.write_all(&archive_data)
        .map_err(PatcherError::OutputError)?;

    // Write size (8 bytes, little-endian)
    let size_bytes = (archive_data.len() as u64).to_le_bytes();
    file.write_all(&size_bytes)
        .map_err(PatcherError::OutputError)?;

    // Write magic marker
    file.write_all(MAGIC_MARKER)
        .map_err(PatcherError::OutputError)?;

    file.flush().map_err(PatcherError::OutputError)?;

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

    let total_size = stub_data.len() + archive_data.len() + 16;
    println!("done");
    println!();
    println!("Created: {} ({} bytes)", output.display(), total_size);

    Ok(())
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
