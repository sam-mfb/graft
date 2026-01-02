//! Path restriction checks to prevent patching sensitive locations.
//!
//! When `allow_restricted` is false in the manifest, these checks prevent:
//! - Path traversal attacks (../)
//! - Patching system directories
//! - Patching executable files
//! - Patching inside .app bundles (macOS)

use crate::utils::manifest::Manifest;
use std::path::Path;

/// A violation of path restrictions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestrictionViolation {
    /// Path contains traversal sequences like ../
    PathTraversal { path: String },
    /// Path resolves to a protected system location
    ProtectedPath { path: String, reason: String },
    /// File has a blocked extension (executable)
    BlockedExtension { path: String, extension: String },
}

impl std::fmt::Display for RestrictionViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RestrictionViolation::PathTraversal { path } => {
                write!(f, "{}: Path traversal not allowed", path)
            }
            RestrictionViolation::ProtectedPath { path, reason } => {
                write!(f, "{}: {}", path, reason)
            }
            RestrictionViolation::BlockedExtension { path, extension } => {
                write!(f, "{}: Cannot patch executable files ({})", path, extension)
            }
        }
    }
}

/// Check all paths in a manifest against restrictions.
///
/// If `manifest.allow_restricted` is true, all checks are bypassed.
/// Returns Ok(()) if all paths are allowed, Err with violations if any are blocked.
pub fn check_manifest(
    manifest: &Manifest,
    target_dir: &Path,
) -> Result<(), Vec<RestrictionViolation>> {
    if manifest.allow_restricted {
        return Ok(()); // Restrictions disabled for this patch
    }

    let mut violations = Vec::new();

    for entry in &manifest.entries {
        let file = entry.file();
        if let Err(v) = check_path(file, target_dir) {
            violations.push(v);
        }
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

/// Check a single file path against all restrictions.
fn check_path(file: &str, target_dir: &Path) -> Result<(), RestrictionViolation> {
    check_path_traversal(file)?;
    check_blocked_extension(file)?;
    check_protected_path(file, target_dir)?;
    Ok(())
}

/// Check for path traversal sequences.
fn check_path_traversal(file: &str) -> Result<(), RestrictionViolation> {
    // Check for .. components
    for component in Path::new(file).components() {
        if let std::path::Component::ParentDir = component {
            return Err(RestrictionViolation::PathTraversal {
                path: file.to_string(),
            });
        }
    }

    // Also check for explicit .. in the string (handles edge cases)
    if file.contains("..") {
        return Err(RestrictionViolation::PathTraversal {
            path: file.to_string(),
        });
    }

    Ok(())
}

/// Blocked file extensions by platform.
#[cfg(target_os = "windows")]
const BLOCKED_EXTENSIONS_WINDOWS: &[&str] = &[
    ".exe", ".dll", ".sys", ".com", ".bat", ".cmd", ".ps1", ".msi", ".scr",
];

#[cfg(target_os = "macos")]
const BLOCKED_EXTENSIONS_MACOS: &[&str] = &[".dylib", ".bundle", ".kext"];

#[cfg(target_os = "linux")]
const BLOCKED_EXTENSIONS_LINUX: &[&str] = &[".so", ".ko"];

const BLOCKED_EXTENSIONS_CROSS_PLATFORM: &[&str] = &[".sh", ".bin"];

/// Check if a file has a blocked extension.
fn check_blocked_extension(file: &str) -> Result<(), RestrictionViolation> {
    let file_lower = file.to_lowercase();

    // Check cross-platform extensions
    for ext in BLOCKED_EXTENSIONS_CROSS_PLATFORM {
        if file_lower.ends_with(ext) {
            return Err(RestrictionViolation::BlockedExtension {
                path: file.to_string(),
                extension: ext.to_string(),
            });
        }
    }

    // Check platform-specific extensions
    #[cfg(target_os = "windows")]
    for ext in BLOCKED_EXTENSIONS_WINDOWS {
        if file_lower.ends_with(ext) {
            return Err(RestrictionViolation::BlockedExtension {
                path: file.to_string(),
                extension: ext.to_string(),
            });
        }
    }

    #[cfg(target_os = "macos")]
    for ext in BLOCKED_EXTENSIONS_MACOS {
        if file_lower.ends_with(ext) {
            return Err(RestrictionViolation::BlockedExtension {
                path: file.to_string(),
                extension: ext.to_string(),
            });
        }
    }

    #[cfg(target_os = "linux")]
    for ext in BLOCKED_EXTENSIONS_LINUX {
        if file_lower.ends_with(ext) {
            return Err(RestrictionViolation::BlockedExtension {
                path: file.to_string(),
                extension: ext.to_string(),
            });
        }
    }

    Ok(())
}

/// Check if the resolved path is in a protected system location.
fn check_protected_path(file: &str, target_dir: &Path) -> Result<(), RestrictionViolation> {
    let target_path = target_dir.join(file);

    // Try to canonicalize to get the real path
    // If canonicalize fails (file doesn't exist yet), use the joined path
    let resolved = target_path.canonicalize().unwrap_or(target_path);

    if let Some(reason) = is_protected_path(&resolved) {
        return Err(RestrictionViolation::ProtectedPath {
            path: file.to_string(),
            reason: reason.to_string(),
        });
    }

    Ok(())
}

/// Check if a path is in a protected location (platform-specific).
#[cfg(target_os = "macos")]
fn is_protected_path(path: &Path) -> Option<&'static str> {
    let path_str = path.to_string_lossy();

    // Check for .app bundles
    if path_str.contains(".app/") {
        return Some("Cannot patch inside .app bundles");
    }

    // Check system directories
    let protected_prefixes = [
        "/System",
        "/Library",
        "/usr",
        "/bin",
        "/sbin",
        "/var",
        "/etc",
        "/private",
    ];

    for prefix in protected_prefixes {
        if path_str.starts_with(prefix) {
            // Exception: /usr/local is allowed
            if prefix == "/usr" && path_str.starts_with("/usr/local") {
                continue;
            }
            return Some("Cannot patch system directories");
        }
    }

    // Check ~/Library (except Application Support)
    if let Some(home) = dirs::home_dir() {
        let home_library = home.join("Library");
        if path.starts_with(&home_library) {
            let app_support = home_library.join("Application Support");
            if !path.starts_with(&app_support) {
                return Some("Cannot patch ~/Library (except Application Support)");
            }
        }
    }

    None
}

#[cfg(target_os = "windows")]
fn is_protected_path(path: &Path) -> Option<&'static str> {
    let path_str = path.to_string_lossy().to_lowercase();

    // Check Windows system directories
    let protected_patterns = [
        "c:\\windows",
        "c:\\program files",
        "c:\\program files (x86)",
        "c:\\programdata",
    ];

    for pattern in protected_patterns {
        if path_str.starts_with(pattern) {
            return Some("Cannot patch Windows system directories");
        }
    }

    // Check System32, SysWOW64
    if path_str.contains("\\system32\\") || path_str.contains("\\syswow64\\") {
        return Some("Cannot patch Windows system directories");
    }

    None
}

#[cfg(target_os = "linux")]
fn is_protected_path(path: &Path) -> Option<&'static str> {
    let path_str = path.to_string_lossy();

    let protected_prefixes = [
        "/usr", "/bin", "/sbin", "/lib", "/lib64", "/etc", "/var", "/boot", "/opt",
    ];

    for prefix in protected_prefixes {
        if path_str.starts_with(prefix) {
            // Exception: /usr/local and /var/games are allowed
            if prefix == "/usr" && path_str.starts_with("/usr/local") {
                continue;
            }
            if prefix == "/var" && path_str.starts_with("/var/games") {
                continue;
            }
            return Some("Cannot patch system directories");
        }
    }

    None
}

// Fallback for other platforms
#[cfg(not(any(target_os = "macos", target_os = "windows", target_os = "linux")))]
fn is_protected_path(_path: &Path) -> Option<&'static str> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::manifest::ManifestEntry;

    #[test]
    fn path_traversal_is_blocked() {
        assert!(check_path_traversal("../etc/passwd").is_err());
        assert!(check_path_traversal("data/../../../etc/passwd").is_err());
        assert!(check_path_traversal("..\\windows\\system32").is_err());
    }

    #[test]
    fn normal_paths_are_allowed() {
        assert!(check_path_traversal("data/game.dat").is_ok());
        assert!(check_path_traversal("assets/textures/sky.png").is_ok());
        assert!(check_path_traversal("config.json").is_ok());
    }

    #[test]
    fn blocked_extensions_cross_platform() {
        assert!(check_blocked_extension("script.sh").is_err());
        assert!(check_blocked_extension("program.bin").is_err());
    }

    #[test]
    fn normal_extensions_allowed() {
        assert!(check_blocked_extension("data.dat").is_ok());
        assert!(check_blocked_extension("texture.png").is_ok());
        assert!(check_blocked_extension("config.json").is_ok());
        assert!(check_blocked_extension("readme.txt").is_ok());
    }

    #[test]
    fn allow_restricted_bypasses_all_checks() {
        let manifest = Manifest {
            version: 1,
            title: None,
            allow_restricted: true,
            entries: vec![ManifestEntry::Patch {
                file: "../../../etc/passwd".to_string(),
                original_hash: "a".to_string(),
                diff_hash: "b".to_string(),
                final_hash: "c".to_string(),
            }],
        };

        let result = check_manifest(&manifest, Path::new("/tmp"));
        assert!(result.is_ok());
    }

    #[test]
    fn restricted_manifest_blocks_traversal() {
        let manifest = Manifest {
            version: 1,
            title: None,
            allow_restricted: false,
            entries: vec![ManifestEntry::Patch {
                file: "../secret.txt".to_string(),
                original_hash: "a".to_string(),
                diff_hash: "b".to_string(),
                final_hash: "c".to_string(),
            }],
        };

        let result = check_manifest(&manifest, Path::new("/tmp"));
        assert!(result.is_err());
        let violations = result.unwrap_err();
        assert_eq!(violations.len(), 1);
        assert!(matches!(
            &violations[0],
            RestrictionViolation::PathTraversal { .. }
        ));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_app_bundle_is_blocked() {
        let path = Path::new("/Applications/Safari.app/Contents/MacOS/Safari");
        assert!(is_protected_path(path).is_some());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_system_dirs_are_blocked() {
        assert!(is_protected_path(Path::new("/System/Library/file")).is_some());
        assert!(is_protected_path(Path::new("/usr/bin/ls")).is_some());
        assert!(is_protected_path(Path::new("/Library/Preferences/file")).is_some());
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn macos_usr_local_is_allowed() {
        assert!(is_protected_path(Path::new("/usr/local/bin/myapp")).is_none());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_system_dirs_are_blocked() {
        assert!(is_protected_path(Path::new("/usr/bin/ls")).is_some());
        assert!(is_protected_path(Path::new("/etc/passwd")).is_some());
        assert!(is_protected_path(Path::new("/var/log/syslog")).is_some());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn linux_usr_local_is_allowed() {
        assert!(is_protected_path(Path::new("/usr/local/bin/myapp")).is_none());
    }

    #[cfg(target_os = "windows")]
    #[test]
    fn windows_system_dirs_are_blocked() {
        assert!(is_protected_path(Path::new("C:\\Windows\\System32\\cmd.exe")).is_some());
        assert!(is_protected_path(Path::new("C:\\Program Files\\app")).is_some());
    }
}
