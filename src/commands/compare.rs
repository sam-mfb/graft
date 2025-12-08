use std::io;
use std::path::Path;

use crate::utils::hash::hash_file;

pub struct CompareResult {
    pub hash1: String,
    pub hash2: String,
    pub matches: bool,
}

pub fn run(file1: &Path, file2: &Path) -> io::Result<CompareResult> {
    let hash1 = hash_file(file1)?;
    let hash2 = hash_file(file2)?;
    let matches = hash1 == hash2;
    Ok(CompareResult { hash1, hash2, matches })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn identical_files_match() {
        let mut file1 = NamedTempFile::new().unwrap();
        let mut file2 = NamedTempFile::new().unwrap();

        file1.write_all(b"same content").unwrap();
        file2.write_all(b"same content").unwrap();

        let result = run(file1.path(), file2.path()).unwrap();

        assert!(result.matches);
        assert_eq!(result.hash1, result.hash2);
    }

    #[test]
    fn different_files_do_not_match() {
        let mut file1 = NamedTempFile::new().unwrap();
        let mut file2 = NamedTempFile::new().unwrap();

        file1.write_all(b"content a").unwrap();
        file2.write_all(b"content b").unwrap();

        let result = run(file1.path(), file2.path()).unwrap();

        assert!(!result.matches);
        assert_ne!(result.hash1, result.hash2);
    }

    #[test]
    fn nonexistent_file_returns_error() {
        let file1 = NamedTempFile::new().unwrap();
        let nonexistent = Path::new("/nonexistent/file.bin");

        let result = run(file1.path(), nonexistent);

        assert!(result.is_err());
    }
}
