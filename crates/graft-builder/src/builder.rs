use crate::archive;
use crate::error::BuildError;
use graft_core::patch;
use graft_core::utils::manifest::PatchInfo;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Build a GUI patcher executable from a patch directory.
///
/// # Arguments
/// * `patch_dir` - Path to the patch directory (containing manifest.json)
/// * `output_dir` - Directory where the built executable will be placed
/// * `name` - Optional name for the executable (defaults to "patcher")
///
/// # Returns
/// Path to the built executable on success.
pub fn build(patch_dir: &Path, output_dir: &Path, name: Option<&str>) -> Result<PathBuf, BuildError> {
    // Step 1: Validate patch directory
    let manifest = patch::validate_patch_dir(patch_dir)?;
    let patch_info = PatchInfo::from_manifest(&manifest);
    let patcher_name = name.unwrap_or("patcher");

    println!(
        "Building patcher for patch v{} ({} entries: {} patches, {} additions, {} deletions)...",
        patch_info.version,
        patch_info.entry_count,
        patch_info.patches,
        patch_info.additions,
        patch_info.deletions
    );

    // Step 2: Find workspace root
    let workspace_root = find_workspace_root()?;
    let graft_gui_dir = workspace_root.join("crates/graft-gui");
    let archive_path = graft_gui_dir.join("patch_data.tar.gz");

    // Step 3: Create the archive
    println!("Creating patch archive...");
    let archive_data =
        archive::create_archive(patch_dir).map_err(BuildError::ArchiveCreationFailed)?;

    archive::write_archive(&archive_data, &archive_path)
        .map_err(BuildError::ArchiveCreationFailed)?;

    // Step 4: Run cargo build
    println!("Building graft-gui with embedded patch...");
    let build_result = run_cargo_build(&workspace_root);

    // Step 5: Clean up the archive file (do this before checking build result)
    // We want to clean up even if build fails
    if let Err(e) = cleanup_archive(&archive_path) {
        eprintln!("Warning: failed to clean up archive: {}", e);
    }

    // Now check build result
    build_result?;

    // Step 6: Create output directory
    fs::create_dir_all(output_dir).map_err(|e| BuildError::OutputDirCreationFailed {
        path: output_dir.to_path_buf(),
        source: e,
    })?;

    // Step 7: Copy binary to output
    let binary_name = get_binary_name(patcher_name);
    let source_binary = get_release_binary_path(&workspace_root);
    let dest_binary = output_dir.join(&binary_name);

    if !source_binary.exists() {
        return Err(BuildError::BinaryNotFound(source_binary));
    }

    fs::copy(&source_binary, &dest_binary).map_err(|e| BuildError::CopyFailed {
        from: source_binary.clone(),
        to: dest_binary.clone(),
        source: e,
    })?;

    println!("Build complete!");
    Ok(dest_binary)
}

/// Find the workspace root by looking for Cargo.toml with [workspace]
fn find_workspace_root() -> Result<PathBuf, BuildError> {
    // Try using CARGO_MANIFEST_DIR if available (set during cargo run)
    if let Ok(manifest_dir) = std::env::var("CARGO_MANIFEST_DIR") {
        let manifest_path = PathBuf::from(manifest_dir);
        // graft-builder is in crates/graft-builder, so workspace is ../..
        if let Some(workspace) = manifest_path.parent().and_then(|p| p.parent()) {
            if workspace.join("Cargo.toml").exists() {
                return Ok(workspace.to_path_buf());
            }
        }
    }

    // Fallback: use cargo locate-project
    let output = Command::new("cargo")
        .args(["locate-project", "--workspace", "--message-format=plain"])
        .output()
        .map_err(|_| BuildError::WorkspaceNotFound)?;

    if !output.status.success() {
        return Err(BuildError::WorkspaceNotFound);
    }

    let path_str = String::from_utf8_lossy(&output.stdout);
    let cargo_toml = PathBuf::from(path_str.trim());

    cargo_toml
        .parent()
        .map(|p| p.to_path_buf())
        .ok_or(BuildError::WorkspaceNotFound)
}

/// Run cargo build for graft-gui with embedded_patch feature
fn run_cargo_build(workspace_root: &Path) -> Result<(), BuildError> {
    let output = Command::new("cargo")
        .args([
            "build",
            "--release",
            "--package",
            "graft-gui",
            "--features",
            "embedded_patch",
        ])
        .current_dir(workspace_root)
        .output()
        .map_err(|e| BuildError::CargoBuildFailed {
            exit_code: None,
            stderr: e.to_string(),
        })?;

    if !output.status.success() {
        return Err(BuildError::CargoBuildFailed {
            exit_code: output.status.code(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(())
}

/// Get the platform-appropriate binary name
fn get_binary_name(name: &str) -> String {
    if cfg!(target_os = "windows") {
        format!("{}.exe", name)
    } else {
        name.to_string()
    }
}

/// Get the path to the release binary
fn get_release_binary_path(workspace_root: &Path) -> PathBuf {
    let binary_name = if cfg!(target_os = "windows") {
        "graft-gui.exe"
    } else {
        "graft-gui"
    };

    workspace_root.join("target/release").join(binary_name)
}

/// Clean up the temporary archive file
fn cleanup_archive(archive_path: &Path) -> Result<(), BuildError> {
    if archive_path.exists() {
        fs::remove_file(archive_path).map_err(BuildError::CleanupFailed)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn get_binary_name_adds_exe_on_windows() {
        let name = get_binary_name("patcher");
        if cfg!(target_os = "windows") {
            assert_eq!(name, "patcher.exe");
        } else {
            assert_eq!(name, "patcher");
        }
    }

    #[test]
    fn find_workspace_root_works() {
        // This test only works when running via cargo test
        let result = find_workspace_root();
        assert!(result.is_ok());
        let root = result.unwrap();
        assert!(root.join("Cargo.toml").exists());
        assert!(root.join("crates/graft-builder").exists());
    }
}
