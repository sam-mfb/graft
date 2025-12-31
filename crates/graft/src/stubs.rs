//! Stub binary provider - handles embedded, cached, and downloaded stubs.
//!
//! This module provides stub binaries for patcher creation. Stubs can come from:
//! 1. Embedded in the binary (when compiled with `embedded-stubs` feature)
//! 2. Cached locally from a previous download
//! 3. Downloaded from GitHub releases on demand

use crate::targets::{self, Target};
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

/// Errors that can occur when getting stubs.
#[derive(Debug)]
pub enum StubError {
    /// The requested target is not available.
    TargetNotAvailable(String),
    /// Failed to download the stub.
    DownloadFailed(String),
    /// Cache directory error.
    CacheError(io::Error),
}

impl std::fmt::Display for StubError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            StubError::TargetNotAvailable(t) => write!(f, "Stub not available for target: {}", t),
            StubError::DownloadFailed(msg) => write!(f, "Failed to download stub: {}", msg),
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
/// 2. Native stub (if compiled with `native-stub` feature and target matches)
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

    // 2. Try native stub (if compiled with native-stub feature and target matches)
    #[cfg(feature = "native-stub")]
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

/// Check if a stub is available for the target (without downloading).
pub fn is_stub_available(target: &Target) -> bool {
    #[cfg(feature = "embedded-stubs")]
    {
        if get_embedded_stub(target).is_some() {
            return true;
        }
    }

    #[cfg(feature = "native-stub")]
    {
        if let Some(current) = targets::current_target() {
            if current.name == target.name && get_native_stub().is_some() {
                return true;
            }
        }
    }

    // Check cache
    if let Ok(cache) = cache_dir() {
        let path = cache.join(targets::stub_filename(target));
        if path.exists() {
            return true;
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

// Embedded stubs (when compiled with embedded-stubs feature)
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
        "macos-x64" => Some(include_bytes!(concat!(
            env!("GRAFT_STUBS_DIR"),
            "/graft-gui-stub-macos-x64"
        ))),
        "macos-arm64" => Some(include_bytes!(concat!(
            env!("GRAFT_STUBS_DIR"),
            "/graft-gui-stub-macos-arm64"
        ))),
        _ => None,
    }
}

// Native stub (when compiled with native-stub feature)
#[cfg(feature = "native-stub")]
fn get_native_stub() -> Option<&'static [u8]> {
    Some(include_bytes!(env!("GRAFT_NATIVE_STUB")))
}
