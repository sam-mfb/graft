//! Runtime self-reading for appended patch data.
//!
//! This module allows the patcher to read patch data that has been
//! appended to the end of its own executable. This enables creating
//! patchers by simple file concatenation rather than recompilation.
//!
//! # Binary Format
//!
//! ```text
//! ┌─────────────────────────┐
//! │   Executable Code       │  ← Original stub binary
//! ├─────────────────────────┤
//! │   Patch Archive         │  ← tar.gz data (variable size)
//! ├─────────────────────────┤
//! │   Size (8 bytes)        │  ← Archive size as u64 LE
//! ├─────────────────────────┤
//! │   Magic (8 bytes)       │  ← "GRAFTPCH"
//! └─────────────────────────┘
//! ```

use graft_core::archive::MAGIC_MARKER;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
#[cfg(target_os = "macos")]
use std::fs;

/// Errors that can occur when reading appended patch data.
#[derive(Debug)]
pub enum SelfReadError {
    /// No appended data was found (no magic marker).
    NoAppendedData,
    /// The size field is invalid or corrupt.
    InvalidSize,
    /// An I/O error occurred.
    IoError(io::Error),
}

impl std::fmt::Display for SelfReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SelfReadError::NoAppendedData => write!(f, "No appended patch data found"),
            SelfReadError::InvalidSize => write!(f, "Invalid size in appended data"),
            SelfReadError::IoError(e) => write!(f, "I/O error reading appended data: {}", e),
        }
    }
}

impl std::error::Error for SelfReadError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SelfReadError::IoError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<io::Error> for SelfReadError {
    fn from(e: io::Error) -> Self {
        SelfReadError::IoError(e)
    }
}

/// Read appended patch data from the current executable.
///
/// This function reads the executable file itself, looks for the magic
/// marker at the end, and extracts the patch archive data.
///
/// # Returns
///
/// Returns the patch archive bytes (tar.gz format) if found,
/// or an error if no appended data is present or is invalid.
pub fn read_appended_data() -> Result<Vec<u8>, SelfReadError> {
    let exe_path = std::env::current_exe().map_err(SelfReadError::IoError)?;

    let mut file = File::open(&exe_path)?;
    let file_len = file.metadata()?.len();

    // Need at least magic (8) + size (8) = 16 bytes
    if file_len < 16 {
        return Err(SelfReadError::NoAppendedData);
    }

    // Read magic marker (last 8 bytes)
    file.seek(SeekFrom::End(-8))?;
    let mut magic = [0u8; 8];
    file.read_exact(&mut magic)?;

    if &magic != MAGIC_MARKER {
        return Err(SelfReadError::NoAppendedData);
    }

    // Read size (8 bytes before magic)
    file.seek(SeekFrom::End(-16))?;
    let mut size_bytes = [0u8; 8];
    file.read_exact(&mut size_bytes)?;
    let patch_size = u64::from_le_bytes(size_bytes);

    // Validate size
    if patch_size == 0 {
        return Err(SelfReadError::InvalidSize);
    }
    if patch_size > file_len - 16 {
        return Err(SelfReadError::InvalidSize);
    }

    // Read patch data
    let patch_start = file_len - 16 - patch_size;
    file.seek(SeekFrom::Start(patch_start))?;

    let mut patch_data = vec![0u8; patch_size as usize];
    file.read_exact(&mut patch_data)?;

    Ok(patch_data)
}

/// Read patch data from the Resources folder in a macOS .app bundle.
///
/// On macOS, the executable is at:
///   `MyApp.app/Contents/MacOS/graft-gui`
///
/// And patch data is stored at:
///   `MyApp.app/Contents/Resources/patch.data`
///
/// This approach preserves the executable's code signature.
#[cfg(target_os = "macos")]
pub fn read_resources_patch_data() -> Result<Vec<u8>, SelfReadError> {
    let exe_path = std::env::current_exe().map_err(SelfReadError::IoError)?;

    // exe_path: /path/to/MyApp.app/Contents/MacOS/graft-gui
    // target:   /path/to/MyApp.app/Contents/Resources/patch.data
    let contents_dir = exe_path
        .parent() // MacOS/
        .and_then(|p| p.parent()) // Contents/
        .ok_or_else(|| {
            SelfReadError::IoError(io::Error::new(
                io::ErrorKind::NotFound,
                "Could not find Contents directory",
            ))
        })?;

    let patch_data_path = contents_dir.join("Resources").join("patch.data");

    if !patch_data_path.exists() {
        return Err(SelfReadError::NoAppendedData);
    }

    fs::read(&patch_data_path).map_err(SelfReadError::IoError)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_appended_data_returns_error() {
        // Current test binary has no appended data
        let result = read_appended_data();
        assert!(matches!(result, Err(SelfReadError::NoAppendedData)));
    }
}
