//! Stub binary provider - handles embedded, cached, and downloaded stubs.
//!
//! This module provides stub binaries for patcher creation. Stubs can come from:
//! 1. Embedded in the binary (when compiled with `embedded-stubs` feature)
//! 2. Cached locally from a previous download
//! 3. Downloaded from GitHub releases on demand
//!
//! For macOS targets, stubs are distributed as .app bundles (zipped) which are
//! extracted and cached. Use `get_stub_bundle()` for macOS targets.

use crate::targets::{self, Target};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};

/// Errors that can occur when getting stubs.
#[derive(Debug)]
pub enum StubError {
    /// The requested target is not available.
    TargetNotAvailable(String),
    /// Failed to download the stub.
    DownloadFailed(String),
    /// Failed to extract bundle.
    ExtractFailed(String),
    /// Cache directory error.
    CacheError(io::Error),
}

impl std::fmt::Display for StubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StubError::TargetNotAvailable(t) => write!(f, "Stub not available for target: {}", t),
            StubError::DownloadFailed(msg) => write!(f, "Failed to download stub: {}", msg),
            StubError::ExtractFailed(msg) => write!(f, "Failed to extract bundle: {}", msg),
            StubError::CacheError(e) => write!(f, "Cache error: {}", e),
        }
    }
}

impl std::error::Error for StubError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            StubError::CacheError(e) => Some(e),
            _ => None,
        }
    }
}

/// Get the cache directory for stubs.
fn cache_dir() -> io::Result<PathBuf> {
    let base = dirs::cache_dir()
        .ok_or_else(|| io::Error::new(io::ErrorKind::NotFound, "No cache directory found"))?;
    let path = base.join("graft").join("stubs");
    fs::create_dir_all(&path)?;
    Ok(path)
}

/// Get stub bytes for a target.
///
/// Priority:
/// 1. Embedded stubs (if compiled with `embedded-stubs` feature)
/// 2. Native stub (if compiled with `native-stub` feature on Linux and target matches)
/// 3. Cached stub from previous download
/// 4. Download from GitHub releases
pub fn get_stub(target: &Target) -> Result<Vec<u8>, StubError> {
    // 1. Try embedded stubs (if compiled with embedded-stubs feature)
    #[cfg(feature = "embedded-stubs")]
    {
        if let Some(data) = get_embedded_stub(target) {
            return Ok(data.to_vec());
        }
    }

    // 2. Try native stub (Linux only with native-stub feature)
    #[cfg(all(feature = "native-stub", target_os = "linux"))]
    {
        if let Some(current) = targets::current_target() {
            if current.name == target.name {
                if let Some(data) = get_native_stub() {
                    return Ok(data.to_vec());
                }
            }
        }
    }

    // 3. Try cached stub
    if let Ok(data) = get_cached_stub(target) {
        return Ok(data);
    }

    // 4. Download stub
    download_stub(target)
}

/// Get stub bundle path for a macOS target.
///
/// Returns the path to the extracted .app bundle. The bundle is cached
/// after first download/extraction.
///
/// Priority:
/// 1. Embedded bundle (if compiled with `embedded-stubs` feature)
/// 2. Cached extracted bundle
/// 3. Download from GitHub releases and extract
pub fn get_stub_bundle(target: &Target) -> Result<PathBuf, StubError> {
    if !target.stub_is_bundle {
        return Err(StubError::TargetNotAvailable(format!(
            "{} is not a bundle target",
            target.name
        )));
    }

    let cache = cache_dir().map_err(StubError::CacheError)?;
    let bundle_name = format!("graft-gui-stub-{}.app", target.name);
    let bundle_path = cache.join(&bundle_name);

    // 1. Try embedded bundle (extract to cache if not already there)
    #[cfg(feature = "embedded-stubs")]
    {
        if let Some(zip_data) = get_embedded_stub_bundle(target) {
            if !bundle_path.exists() {
                extract_zip(zip_data, &bundle_path)?;
            }
            return Ok(bundle_path);
        }
    }

    // 2. Check if already extracted in cache
    if bundle_path.exists() && bundle_path.is_dir() {
        return Ok(bundle_path);
    }

    // 3. Download and extract
    download_and_extract_bundle(target, &bundle_path)?;

    Ok(bundle_path)
}

/// Check if a stub is available for the target (without downloading).
pub fn is_stub_available(target: &Target) -> bool {
    #[cfg(feature = "embedded-stubs")]
    {
        // Check for embedded binary stub (non-macOS)
        if get_embedded_stub(target).is_some() {
            return true;
        }
        // Check for embedded bundle stub (macOS)
        if get_embedded_stub_bundle(target).is_some() {
            return true;
        }
    }

    #[cfg(all(feature = "native-stub", target_os = "linux"))]
    {
        if let Some(current) = targets::current_target() {
            if current.name == target.name && get_native_stub().is_some() {
                return true;
            }
        }
    }

    // Check cache
    if let Ok(cache) = cache_dir() {
        if target.stub_is_bundle {
            // Check for extracted bundle
            let bundle_path = cache.join(format!("graft-gui-stub-{}.app", target.name));
            if bundle_path.exists() && bundle_path.is_dir() {
                return true;
            }
        } else {
            // Check for binary stub
            let path = cache.join(targets::stub_filename(target));
            if path.exists() {
                return true;
            }
        }
    }

    // Would need to download
    false
}

/// Get cached stub if available.
fn get_cached_stub(target: &Target) -> Result<Vec<u8>, StubError> {
    let cache = cache_dir().map_err(StubError::CacheError)?;
    let path = cache.join(targets::stub_filename(target));

    if path.exists() {
        fs::read(&path).map_err(StubError::CacheError)
    } else {
        Err(StubError::TargetNotAvailable(target.name.to_string()))
    }
}

/// Download stub from GitHub releases and cache it.
///
/// By default, downloads from the "latest" release. Set `GRAFT_STUB_VERSION`
/// environment variable to download a specific version (e.g., "0.1.0").
fn download_stub(target: &Target) -> Result<Vec<u8>, StubError> {
    let filename = targets::stub_filename(target);

    let url = match std::env::var("GRAFT_STUB_VERSION") {
        Ok(version) => format!(
            "https://github.com/sam-mfb/graft/releases/download/v{}/{}",
            version, filename
        ),
        Err(_) => format!(
            "https://github.com/sam-mfb/graft/releases/latest/download/{}",
            filename
        ),
    };

    println!("Downloading stub for {}...", target.name);
    println!("  URL: {}", url);

    // Use ureq for simple HTTP GET
    let response = ureq::get(&url)
        .call()
        .map_err(|e| StubError::DownloadFailed(e.to_string()))?;

    if response.status() != 200 {
        return Err(StubError::DownloadFailed(format!(
            "HTTP {}: {}",
            response.status(),
            url
        )));
    }

    let mut data = Vec::new();
    response
        .into_body()
        .into_reader()
        .read_to_end(&mut data)
        .map_err(|e| StubError::DownloadFailed(e.to_string()))?;

    // Cache for future use
    if let Ok(cache) = cache_dir() {
        let path = cache.join(&filename);
        if let Err(e) = fs::write(&path, &data) {
            eprintln!("Warning: Failed to cache stub: {}", e);
        } else {
            println!("  Cached at: {}", path.display());
        }
    }

    Ok(data)
}

/// Download and extract a .app bundle stub.
fn download_and_extract_bundle(target: &Target, output_path: &Path) -> Result<(), StubError> {
    let filename = targets::stub_filename(target);

    let url = match std::env::var("GRAFT_STUB_VERSION") {
        Ok(version) => format!(
            "https://github.com/sam-mfb/graft/releases/download/v{}/{}",
            version, filename
        ),
        Err(_) => format!(
            "https://github.com/sam-mfb/graft/releases/latest/download/{}",
            filename
        ),
    };

    println!("Downloading stub bundle for {}...", target.name);
    println!("  URL: {}", url);

    // Download the zip file
    let response = ureq::get(&url)
        .call()
        .map_err(|e| StubError::DownloadFailed(e.to_string()))?;

    if response.status() != 200 {
        return Err(StubError::DownloadFailed(format!(
            "HTTP {}: {}",
            response.status(),
            url
        )));
    }

    let mut zip_data = Vec::new();
    response
        .into_body()
        .into_reader()
        .read_to_end(&mut zip_data)
        .map_err(|e| StubError::DownloadFailed(e.to_string()))?;

    println!("  Downloaded {} bytes, extracting...", zip_data.len());

    // Extract the zip to the output path
    extract_zip(&zip_data, output_path)?;

    println!("  Extracted to: {}", output_path.display());

    Ok(())
}

/// Extract a zip archive to the specified directory.
fn extract_zip(zip_data: &[u8], output_path: &Path) -> Result<(), StubError> {
    use std::io::Cursor;

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

// Embedded stubs (when compiled with embedded-stubs feature)
// Binary stubs for non-macOS platforms
#[cfg(feature = "embedded-stubs")]
fn get_embedded_stub(target: &Target) -> Option<&'static [u8]> {
    match target.name {
        "linux-x64" => Some(include_bytes!(concat!(
            env!("GRAFT_STUBS_DIR"),
            "/graft-gui-stub-linux-x64"
        ))),
        "linux-arm64" => Some(include_bytes!(concat!(
            env!("GRAFT_STUBS_DIR"),
            "/graft-gui-stub-linux-arm64"
        ))),
        "windows-x64" => Some(include_bytes!(concat!(
            env!("GRAFT_STUBS_DIR"),
            "/graft-gui-stub-windows-x64.exe"
        ))),
        // macOS uses bundle stubs, not binary stubs
        _ => None,
    }
}

// Embedded bundle stubs for macOS (zipped .app bundles)
#[cfg(feature = "embedded-stubs")]
fn get_embedded_stub_bundle(target: &Target) -> Option<&'static [u8]> {
    match target.name {
        "macos-x64" => Some(include_bytes!(concat!(
            env!("GRAFT_STUBS_DIR"),
            "/graft-gui-stub-macos-x64.app.zip"
        ))),
        "macos-arm64" => Some(include_bytes!(concat!(
            env!("GRAFT_STUBS_DIR"),
            "/graft-gui-stub-macos-arm64.app.zip"
        ))),
        _ => None,
    }
}

// Native stub (Linux only, when compiled with native-stub feature)
#[cfg(all(feature = "native-stub", target_os = "linux"))]
fn get_native_stub() -> Option<&'static [u8]> {
    Some(include_bytes!(env!("GRAFT_NATIVE_STUB")))
}
