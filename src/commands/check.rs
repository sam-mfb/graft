use std::fs;
use std::io;
use std::path::Path;

use crate::utils::hash::hash_bytes;

pub enum CheckResult {
    Match,
    NoMatch { actual: String },
}

pub fn run(expected: &str, file: &Path) -> io::Result<CheckResult> {
    let data = fs::read(file)?;
    let actual = hash_bytes(&data);
    if actual == expected {
        Ok(CheckResult::Match)
    } else {
        Ok(CheckResult::NoMatch { actual })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_file(content: &[u8]) -> NamedTempFile {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(content).unwrap();
        file
    }

    #[test]
    fn matching_hash_returns_match() {
        let file = create_temp_file(b"test content");
        let hash = crate::utils::hash::hash_bytes(b"test content");

        let result = run(&hash, file.path()).unwrap();

        assert!(matches!(result, CheckResult::Match));
    }

    #[test]
    fn wrong_hash_returns_no_match() {
        let file = create_temp_file(b"test content");
        let wrong_hash = "0000000000000000000000000000000000000000000000000000000000000000";

        let result = run(wrong_hash, file.path()).unwrap();

        match result {
            CheckResult::NoMatch { actual } => {
                assert_ne!(actual, wrong_hash);
            }
            CheckResult::Match => panic!("Expected NoMatch"),
        }
    }

    #[test]
    fn nonexistent_file_returns_error() {
        let nonexistent = Path::new("/nonexistent/file.bin");

        let result = run("somehash", nonexistent);

        assert!(result.is_err());
    }
}
