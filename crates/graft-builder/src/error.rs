use graft_core::patch::PatchError;
use std::fmt;
use std::io;
use std::path::PathBuf;

/// Errors that can occur during the build process
#[derive(Debug)]
pub enum BuildError {
    /// Patch validation failed
    PatchValidation(PatchError),
    /// Failed to create archive
    ArchiveCreationFailed(io::Error),
    /// Failed to create output directory
    OutputDirCreationFailed { path: PathBuf, source: io::Error },
    /// Cargo build failed
    CargoBuildFailed { exit_code: Option<i32>, stderr: String },
    /// graft-gui binary not found after build
    BinaryNotFound(PathBuf),
    /// Failed to copy binary to output
    CopyFailed { from: PathBuf, to: PathBuf, source: io::Error },
    /// Failed to clean up temporary files
    CleanupFailed(io::Error),
    /// Could not determine workspace root
    WorkspaceNotFound,
}

impl fmt::Display for BuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BuildError::PatchValidation(e) => write!(f, "patch validation failed: {}", e),
            BuildError::ArchiveCreationFailed(e) => {
                write!(f, "failed to create patch archive: {}", e)
            }
            BuildError::OutputDirCreationFailed { path, source } => {
                write!(
                    f,
                    "failed to create output directory {}: {}",
                    path.display(),
                    source
                )
            }
            BuildError::CargoBuildFailed { exit_code, stderr } => match exit_code {
                Some(code) => write!(f, "cargo build failed (exit code {}): {}", code, stderr),
                None => write!(f, "cargo build terminated by signal: {}", stderr),
            },
            BuildError::BinaryNotFound(path) => {
                write!(
                    f,
                    "graft-gui binary not found at expected location: {}",
                    path.display()
                )
            }
            BuildError::CopyFailed { from, to, source } => {
                write!(
                    f,
                    "failed to copy {} to {}: {}",
                    from.display(),
                    to.display(),
                    source
                )
            }
            BuildError::CleanupFailed(e) => {
                write!(f, "failed to clean up temporary files: {}", e)
            }
            BuildError::WorkspaceNotFound => {
                write!(f, "could not determine cargo workspace root")
            }
        }
    }
}

impl std::error::Error for BuildError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BuildError::PatchValidation(e) => Some(e),
            BuildError::ArchiveCreationFailed(e) => Some(e),
            BuildError::OutputDirCreationFailed { source, .. } => Some(source),
            BuildError::CopyFailed { source, .. } => Some(source),
            BuildError::CleanupFailed(e) => Some(e),
            _ => None,
        }
    }
}

impl From<PatchError> for BuildError {
    fn from(e: PatchError) -> Self {
        BuildError::PatchValidation(e)
    }
}
