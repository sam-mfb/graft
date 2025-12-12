use std::fs;
use std::io;
use std::path::Path;

use crate::utils::hash::hash_bytes;

pub fn run(file: &Path) -> io::Result<String> {
    let data = fs::read(file)?;
    Ok(hash_bytes(&data))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn create_temp_file(content: &[u8]) -> NamedTempFile {
        let mut file: NamedTempFile = NamedTempFile::new().unwrap();
        file.write_all(content).unwrap();
        file
    }

    #[test]
    fn returns_expected_hash() {
        let file = create_temp_file(b"test content");
        let hash = run(file.path()).unwrap();

        assert_eq!(hash.as_str(), "6ae8a75555209fd6c44157c0aed8016e763ff435a19cf186f76863140143ff72");
    }
}

