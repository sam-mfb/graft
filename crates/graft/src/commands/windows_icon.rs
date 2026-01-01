//! Windows icon embedding for .exe patchers.
//!
//! Uses the editpe crate to embed icons in Windows PE executables.
//! This works cross-platform (can embed icons from Linux/macOS).

use editpe::Image;
use std::path::Path;

/// Errors from Windows icon embedding.
#[derive(Debug)]
pub enum WindowsIconError {
    /// Failed to parse PE executable.
    ParsePE(String),
    /// Failed to embed icon.
    EmbedIcon(String),
    /// Failed to write PE executable.
    WritePE(String),
}

impl std::fmt::Display for WindowsIconError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WindowsIconError::ParsePE(msg) => write!(f, "Failed to parse PE: {}", msg),
            WindowsIconError::EmbedIcon(msg) => write!(f, "Failed to embed icon: {}", msg),
            WindowsIconError::WritePE(msg) => write!(f, "Failed to write PE: {}", msg),
        }
    }
}

impl std::error::Error for WindowsIconError {}

/// Embed an icon into a Windows executable.
///
/// Reads the PNG icon, converts it to ICO format internally,
/// and embeds it as the main application icon.
///
/// # Arguments
/// * `exe_path` - Path to the Windows executable to modify
/// * `icon_path` - Path to the PNG icon file
pub fn embed_icon(exe_path: &Path, icon_path: &Path) -> Result<(), WindowsIconError> {
    // Parse PE image from file
    let mut image =
        Image::parse_file(exe_path).map_err(|e| WindowsIconError::ParsePE(e.to_string()))?;

    // Get or create resource directory
    let mut resources = image.resource_directory().cloned().unwrap_or_default();

    // Set icon from PNG file (editpe handles PNG to ICO conversion)
    let icon_path_str = icon_path
        .to_str()
        .ok_or_else(|| WindowsIconError::EmbedIcon("Invalid icon path".to_string()))?;
    resources
        .set_main_icon_file(icon_path_str)
        .map_err(|e| WindowsIconError::EmbedIcon(e.to_string()))?;

    // Update image with new resources
    image
        .set_resource_directory(resources)
        .map_err(|e| WindowsIconError::EmbedIcon(e.to_string()))?;

    // Write modified executable back to file
    image
        .write_file(exe_path)
        .map_err(|e| WindowsIconError::WritePE(e.to_string()))?;

    Ok(())
}
