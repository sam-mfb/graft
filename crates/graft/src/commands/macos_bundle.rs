//! macOS .app bundle creation and modification.
//!
//! Creates proper macOS application bundles with icons and Info.plist.
//! Also supports modifying existing stub bundles.

use graft_core::archive::MAGIC_MARKER;
use graft_core::patch::{ASSETS_DIR, ICON_FILENAME};
use icns::{IconFamily, Image};
use std::fs::{self, File};
use std::io::{self, BufReader};
use std::path::Path;

/// Info.plist template for macOS .app bundles.
const INFO_PLIST_TEMPLATE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleExecutable</key>
    <string>{executable}</string>
    <key>CFBundleIdentifier</key>
    <string>com.graft.patcher.{identifier}</string>
    <key>CFBundleName</key>
    <string>{name}</string>
    <key>CFBundleDisplayName</key>
    <string>{name}</string>
    <key>CFBundleVersion</key>
    <string>{version}</string>
    <key>CFBundleShortVersionString</key>
    <string>{version}</string>
    <key>CFBundlePackageType</key>
    <string>APPL</string>
    <key>CFBundleIconFile</key>
    <string>AppIcon</string>
    <key>LSMinimumSystemVersion</key>
    <string>10.13</string>
    <key>NSHighResolutionCapable</key>
    <true/>
</dict>
</plist>
"#;

/// Errors from bundle creation.
#[derive(Debug)]
pub enum BundleError {
    /// Failed to create directory structure.
    DirectoryCreation(io::Error),
    /// Failed to write file.
    FileWrite(io::Error),
    /// Failed to read icon.
    IconRead(io::Error),
    /// Failed to process icon.
    IconProcessing(String),
    /// Icon not found in patch.
    IconNotFound,
}

impl std::fmt::Display for BundleError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BundleError::DirectoryCreation(e) => write!(f, "Failed to create directory: {}", e),
            BundleError::FileWrite(e) => write!(f, "Failed to write file: {}", e),
            BundleError::IconRead(e) => write!(f, "Failed to read icon: {}", e),
            BundleError::IconProcessing(msg) => write!(f, "Icon processing failed: {}", msg),
            BundleError::IconNotFound => write!(f, "Icon not found in patch assets"),
        }
    }
}

impl std::error::Error for BundleError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BundleError::DirectoryCreation(e) => Some(e),
            BundleError::FileWrite(e) => Some(e),
            BundleError::IconRead(e) => Some(e),
            _ => None,
        }
    }
}

/// Create a macOS .app bundle.
///
/// # Arguments
/// * `output_path` - Path for the .app bundle (e.g., "MyApp.app")
/// * `executable_data` - The patcher executable bytes
/// * `patch_dir` - Path to the patch directory (for reading icon)
/// * `app_name` - Name for the app (used in executable and plist)
/// * `title` - Display title for the app (from manifest, or defaults to app_name)
/// * `version` - Version string for the app
pub fn create_bundle(
    output_path: &Path,
    executable_data: &[u8],
    patch_dir: &Path,
    app_name: &str,
    title: Option<&str>,
    version: &str,
) -> Result<(), BundleError> {
    // Create bundle directory structure:
    // MyApp.app/
    //   Contents/
    //     MacOS/
    //       MyApp (executable)
    //     Resources/
    //       AppIcon.icns
    //     Info.plist

    let contents_dir = output_path.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    let resources_dir = contents_dir.join("Resources");

    fs::create_dir_all(&macos_dir).map_err(BundleError::DirectoryCreation)?;
    fs::create_dir_all(&resources_dir).map_err(BundleError::DirectoryCreation)?;

    // Write executable
    let executable_path = macos_dir.join(app_name);
    fs::write(&executable_path, executable_data).map_err(BundleError::FileWrite)?;

    // Make executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&executable_path)
            .map_err(BundleError::FileWrite)?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&executable_path, perms).map_err(BundleError::FileWrite)?;
    }

    // Convert and write icon
    let icon_path = patch_dir.join(ASSETS_DIR).join(ICON_FILENAME);
    if icon_path.exists() {
        let icns_path = resources_dir.join("AppIcon.icns");
        convert_png_to_icns(&icon_path, &icns_path)?;
    }

    // Write Info.plist
    let display_name = title.unwrap_or(app_name);
    // Create a safe identifier from app_name (alphanumeric and hyphens only)
    let identifier: String = app_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect();

    let plist_content = INFO_PLIST_TEMPLATE
        .replace("{executable}", app_name)
        .replace("{identifier}", &identifier)
        .replace("{name}", display_name)
        .replace("{version}", version);

    let plist_path = contents_dir.join("Info.plist");
    fs::write(&plist_path, plist_content).map_err(BundleError::FileWrite)?;

    Ok(())
}

/// Convert a PNG file to .icns format.
pub fn convert_png_to_icns(png_path: &Path, icns_path: &Path) -> Result<(), BundleError> {
    let file = File::open(png_path).map_err(BundleError::IconRead)?;
    let reader = BufReader::new(file);

    let image = Image::read_png(reader)
        .map_err(|e| BundleError::IconProcessing(format!("Failed to read PNG: {}", e)))?;

    let mut icon_family = IconFamily::new();
    icon_family.add_icon(&image)
        .map_err(|e| BundleError::IconProcessing(format!("Failed to add icon: {}", e)))?;

    let mut output_file = File::create(icns_path).map_err(BundleError::FileWrite)?;
    icon_family.write(&mut output_file)
        .map_err(|e| BundleError::IconProcessing(format!("Failed to write icns: {}", e)))?;

    Ok(())
}

/// Modify an existing stub bundle to create a patcher.
///
/// # Arguments
/// * `stub_bundle_path` - Path to the stub .app bundle
/// * `output_path` - Path for the output .app bundle
/// * `archive_data` - The patch archive data to append
/// * `patch_dir` - Path to the patch directory (for reading custom icon)
/// * `title` - Display title for the app (from manifest)
/// * `version` - Version string for the app
pub fn modify_bundle(
    stub_bundle_path: &Path,
    output_path: &Path,
    archive_data: &[u8],
    patch_dir: &Path,
    title: Option<&str>,
    version: &str,
) -> Result<usize, BundleError> {
    // 1. Copy stub bundle to output location
    copy_dir_recursive(stub_bundle_path, output_path)?;

    let contents_dir = output_path.join("Contents");
    let macos_dir = contents_dir.join("MacOS");
    let resources_dir = contents_dir.join("Resources");

    // 2. Find and update the executable
    // The executable name is "graft-gui" in the stub bundle
    let executable_path = macos_dir.join("graft-gui");
    if !executable_path.exists() {
        return Err(BundleError::FileWrite(io::Error::new(
            io::ErrorKind::NotFound,
            "Executable not found in stub bundle",
        )));
    }

    // Read existing executable and append patch data
    let mut stub_data = fs::read(&executable_path).map_err(BundleError::FileWrite)?;

    // Append: archive + size (8 bytes LE) + magic (8 bytes)
    stub_data.extend_from_slice(archive_data);
    let size_bytes = (archive_data.len() as u64).to_le_bytes();
    stub_data.extend_from_slice(&size_bytes);
    stub_data.extend_from_slice(MAGIC_MARKER);

    fs::write(&executable_path, &stub_data).map_err(BundleError::FileWrite)?;
    let total_size = stub_data.len();

    // 3. Update Info.plist with custom title and version
    let display_name = title.unwrap_or("Graft Patcher");
    let app_name = output_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("patcher");

    // Create a safe identifier from app_name
    let identifier: String = app_name
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '-' })
        .collect();

    let plist_content = INFO_PLIST_TEMPLATE
        .replace("{executable}", "graft-gui") // Keep original executable name
        .replace("{identifier}", &identifier)
        .replace("{name}", display_name)
        .replace("{version}", version);

    let plist_path = contents_dir.join("Info.plist");
    fs::write(&plist_path, plist_content).map_err(BundleError::FileWrite)?;

    // 4. Replace icon if patch has custom icon
    let custom_icon_path = patch_dir.join(ASSETS_DIR).join(ICON_FILENAME);
    if custom_icon_path.exists() {
        let icns_path = resources_dir.join("AppIcon.icns");
        convert_png_to_icns(&custom_icon_path, &icns_path)?;
    }
    // Otherwise keep the stub's default icon

    Ok(total_size)
}

/// Recursively copy a directory.
fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), BundleError> {
    if !src.is_dir() {
        return Err(BundleError::DirectoryCreation(io::Error::new(
            io::ErrorKind::NotFound,
            format!("Source is not a directory: {}", src.display()),
        )));
    }

    fs::create_dir_all(dst).map_err(BundleError::DirectoryCreation)?;

    for entry in fs::read_dir(src).map_err(BundleError::DirectoryCreation)? {
        let entry = entry.map_err(BundleError::DirectoryCreation)?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());

        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path).map_err(BundleError::FileWrite)?;

            // Preserve executable permissions on Unix
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                if let Ok(metadata) = fs::metadata(&src_path) {
                    let mode = metadata.permissions().mode();
                    if mode & 0o111 != 0 {
                        // Has execute bit
                        let mut perms = fs::metadata(&dst_path)
                            .map_err(BundleError::FileWrite)?
                            .permissions();
                        perms.set_mode(mode);
                        fs::set_permissions(&dst_path, perms).map_err(BundleError::FileWrite)?;
                    }
                }
            }
        }
    }

    Ok(())
}
