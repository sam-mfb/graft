use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "operation", rename_all = "lowercase")]
pub enum ManifestEntry {
    Patch {
        file: String,
        original_hash: String,
        diff_hash: String,
        final_hash: String,
    },
    Add {
        file: String,
        final_hash: String,
    },
    Delete {
        file: String,
        original_hash: String,
    },
}

impl ManifestEntry {
    pub fn file(&self) -> &str {
        match self {
            ManifestEntry::Patch { file, .. } => file,
            ManifestEntry::Add { file, .. } => file,
            ManifestEntry::Delete { file, .. } => file,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// If true, allows patching restricted paths (system dirs, executables).
    /// Default is false for security.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub allow_restricted: bool,
    pub entries: Vec<ManifestEntry>,
}

impl Manifest {
    pub fn new(version: u32, title: Option<String>) -> Self {
        Manifest {
            version,
            title,
            allow_restricted: false,
            entries: Vec::new(),
        }
    }

    pub fn load(path: &Path) -> io::Result<Manifest> {
        let content = fs::read_to_string(path)?;
        serde_json::from_str(&content)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))
    }

    pub fn save(&self, path: &Path) -> io::Result<()> {
        let content = serde_json::to_string_pretty(self)
            .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
        fs::write(path, content)
    }
}

impl Default for Manifest {
    fn default() -> Self {
        Self::new(1, None)
    }
}

/// Patch metadata extracted from manifest
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchInfo {
    pub version: u32,
    pub title: Option<String>,
    pub entry_count: usize,
    pub patches: usize,
    pub additions: usize,
    pub deletions: usize,
}

impl PatchInfo {
    pub fn from_manifest(manifest: &Manifest) -> Self {
        let mut patches = 0;
        let mut additions = 0;
        let mut deletions = 0;
        for entry in &manifest.entries {
            match entry {
                ManifestEntry::Patch { .. } => patches += 1,
                ManifestEntry::Add { .. } => additions += 1,
                ManifestEntry::Delete { .. } => deletions += 1,
            }
        }
        PatchInfo {
            version: manifest.version,
            title: manifest.title.clone(),
            entry_count: manifest.entries.len(),
            patches,
            additions,
            deletions,
        }
    }

    /// Mock patch info for demo mode
    pub fn mock() -> Self {
        PatchInfo {
            version: 1,
            title: Some("Graft Patcher (Demo)".to_string()),
            entry_count: 42,
            patches: 35,
            additions: 5,
            deletions: 2,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::NamedTempFile;

    #[test]
    fn roundtrip_serialization() {
        let manifest = Manifest {
            version: 1,
            title: Some("Test Patcher".to_string()),
            allow_restricted: false,
            entries: vec![
                ManifestEntry::Patch {
                    file: "game.bin".to_string(),
                    original_hash: "abc123".to_string(),
                    diff_hash: "def456".to_string(),
                    final_hash: "ghi789".to_string(),
                },
                ManifestEntry::Add {
                    file: "new_asset.bin".to_string(),
                    final_hash: "jkl012".to_string(),
                },
                ManifestEntry::Delete {
                    file: "old_asset.bin".to_string(),
                    original_hash: "mno345".to_string(),
                },
            ],
        };

        let temp_file = NamedTempFile::new().unwrap();
        manifest.save(temp_file.path()).unwrap();

        let loaded = Manifest::load(temp_file.path()).unwrap();
        assert_eq!(manifest, loaded);
    }

    #[test]
    fn load_from_json_string() {
        let json = r#"{
            "version": 1,
            "entries": [
                {
                    "operation": "patch",
                    "file": "test.bin",
                    "original_hash": "aaa",
                    "diff_hash": "bbb",
                    "final_hash": "ccc"
                }
            ]
        }"#;

        let temp_file = NamedTempFile::new().unwrap();
        fs::write(temp_file.path(), json).unwrap();

        let manifest = Manifest::load(temp_file.path()).unwrap();
        assert_eq!(manifest.version, 1);
        assert_eq!(manifest.entries.len(), 1);
        assert_eq!(manifest.entries[0].file(), "test.bin");
        assert!(matches!(manifest.entries[0], ManifestEntry::Patch { .. }));
    }

    #[test]
    fn load_missing_file_returns_error() {
        let result = Manifest::load(Path::new("/nonexistent/manifest.json"));
        assert!(result.is_err());
    }

    #[test]
    fn load_malformed_json_returns_error() {
        let temp_file = NamedTempFile::new().unwrap();
        fs::write(temp_file.path(), "not valid json").unwrap();

        let result = Manifest::load(temp_file.path());
        assert!(result.is_err());
    }

    #[test]
    fn save_produces_valid_json() {
        let manifest = Manifest {
            version: 1,
            title: None,
            allow_restricted: false,
            entries: vec![ManifestEntry::Add {
                file: "test.bin".to_string(),
                final_hash: "hash123".to_string(),
            }],
        };

        let temp_file = NamedTempFile::new().unwrap();
        manifest.save(temp_file.path()).unwrap();

        let content = fs::read_to_string(temp_file.path()).unwrap();
        assert!(content.contains("\"operation\": \"add\""));
        assert!(content.contains("\"final_hash\": \"hash123\""));
        assert!(!content.contains("original_hash"));
        assert!(!content.contains("diff_hash"));
    }

    #[test]
    fn file_helper_returns_filename() {
        let patch = ManifestEntry::Patch {
            file: "a.bin".to_string(),
            original_hash: "x".to_string(),
            diff_hash: "y".to_string(),
            final_hash: "z".to_string(),
        };
        let add = ManifestEntry::Add {
            file: "b.bin".to_string(),
            final_hash: "x".to_string(),
        };
        let delete = ManifestEntry::Delete {
            file: "c.bin".to_string(),
            original_hash: "x".to_string(),
        };

        assert_eq!(patch.file(), "a.bin");
        assert_eq!(add.file(), "b.bin");
        assert_eq!(delete.file(), "c.bin");
    }
}
