//! Target platform definitions for patcher stub binaries.

use std::fmt;

/// A target platform for patcher stubs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    /// Short name (e.g., "linux-x64")
    pub name: &'static str,
    /// Rust target triple
    pub triple: &'static str,
    /// Binary suffix (e.g., ".exe" for Windows)
    pub binary_suffix: &'static str,
}

impl fmt::Display for Target {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

pub const LINUX_X64: Target = Target {
    name: "linux-x64",
    triple: "x86_64-unknown-linux-gnu",
    binary_suffix: "",
};

pub const LINUX_ARM64: Target = Target {
    name: "linux-arm64",
    triple: "aarch64-unknown-linux-gnu",
    binary_suffix: "",
};

pub const WINDOWS_X64: Target = Target {
    name: "windows-x64",
    triple: "x86_64-pc-windows-gnu",
    binary_suffix: ".exe",
};

pub const MACOS_X64: Target = Target {
    name: "macos-x64",
    triple: "x86_64-apple-darwin",
    binary_suffix: "",
};

pub const MACOS_ARM64: Target = Target {
    name: "macos-arm64",
    triple: "aarch64-apple-darwin",
    binary_suffix: "",
};

/// All available targets.
pub const ALL_TARGETS: &[Target] = &[
    LINUX_X64,
    LINUX_ARM64,
    WINDOWS_X64,
    MACOS_X64,
    MACOS_ARM64,
];

/// Parse a target name string into a Target.
pub fn parse_target(name: &str) -> Option<Target> {
    match name.to_lowercase().as_str() {
        "linux-x64" | "linux-x86_64" => Some(LINUX_X64),
        "linux-arm64" | "linux-aarch64" => Some(LINUX_ARM64),
        "windows-x64" | "windows" => Some(WINDOWS_X64),
        "macos-x64" | "macos-x86_64" | "darwin-x64" => Some(MACOS_X64),
        "macos-arm64" | "macos-aarch64" | "darwin-arm64" => Some(MACOS_ARM64),
        _ => None,
    }
}

/// Get the current platform's target.
pub fn current_target() -> Option<Target> {
    #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
    return Some(LINUX_X64);

    #[cfg(all(target_os = "linux", target_arch = "aarch64"))]
    return Some(LINUX_ARM64);

    #[cfg(all(target_os = "windows", target_arch = "x86_64"))]
    return Some(WINDOWS_X64);

    #[cfg(all(target_os = "macos", target_arch = "x86_64"))]
    return Some(MACOS_X64);

    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    return Some(MACOS_ARM64);

    #[allow(unreachable_code)]
    None
}

/// Get stub filename for a target.
pub fn stub_filename(target: &Target) -> String {
    format!("graft-gui-stub-{}{}", target.name, target.binary_suffix)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_target_works() {
        assert_eq!(parse_target("linux-x64"), Some(LINUX_X64));
        assert_eq!(parse_target("LINUX-X64"), Some(LINUX_X64));
        assert_eq!(parse_target("windows"), Some(WINDOWS_X64));
        assert_eq!(parse_target("macos-arm64"), Some(MACOS_ARM64));
        assert_eq!(parse_target("invalid"), None);
    }

    #[test]
    fn current_target_returns_some() {
        // Should return Some on any supported platform
        // On Linux x64 (common CI), this should be Some
        #[cfg(all(target_os = "linux", target_arch = "x86_64"))]
        assert_eq!(current_target(), Some(LINUX_X64));
    }

    #[test]
    fn stub_filename_formats_correctly() {
        assert_eq!(stub_filename(&LINUX_X64), "graft-gui-stub-linux-x64");
        assert_eq!(stub_filename(&WINDOWS_X64), "graft-gui-stub-windows-x64.exe");
    }
}
