use std::fs;
use std::io;
use std::path::Path;

use crate::utils::patch::create_patch;

pub fn run(orig: &Path, new: &Path, patch_out: &Path) -> io::Result<()> {
    let orig_data = fs::read(orig)?;
    let new_data = fs::read(new)?;
    let patch_data = create_patch(&orig_data, &new_data)?;
    fs::write(patch_out, patch_data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn creates_patch_file() {
        let mut orig = NamedTempFile::new().unwrap();
        let mut new = NamedTempFile::new().unwrap();
        let patch_out = NamedTempFile::new().unwrap();

        orig.write_all(b"original content").unwrap();
        new.write_all(b"modified content").unwrap();

        run(orig.path(), new.path(), patch_out.path()).unwrap();

        let patch_data = fs::read(patch_out.path()).unwrap();
        assert!(!patch_data.is_empty());
    }

    #[test]
    fn nonexistent_file_returns_error() {
        let new = NamedTempFile::new().unwrap();
        let patch_out = NamedTempFile::new().unwrap();
        let nonexistent = Path::new("/nonexistent/file.bin");

        let result = run(nonexistent, new.path(), patch_out.path());

        assert!(result.is_err());
    }
}
