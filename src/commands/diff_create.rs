use std::fs;
use std::io;
use std::path::Path;

use crate::utils::diff::create_diff;

pub fn run(orig: &Path, new: &Path, diff_out: &Path) -> io::Result<()> {
    let orig_data = fs::read(orig)?;
    let new_data = fs::read(new)?;
    let diff_data = create_diff(&orig_data, &new_data)?;
    fs::write(diff_out, diff_data)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn creates_diff_file() {
        let mut orig = NamedTempFile::new().unwrap();
        let mut new = NamedTempFile::new().unwrap();
        let diff_out = NamedTempFile::new().unwrap();

        orig.write_all(b"original content").unwrap();
        new.write_all(b"modified content").unwrap();

        run(orig.path(), new.path(), diff_out.path()).unwrap();

        let diff_data = fs::read(diff_out.path()).unwrap();
        assert!(!diff_data.is_empty());
    }

    #[test]
    fn nonexistent_file_returns_error() {
        let new = NamedTempFile::new().unwrap();
        let diff_out = NamedTempFile::new().unwrap();
        let nonexistent = Path::new("/nonexistent/file.bin");

        let result = run(nonexistent, new.path(), diff_out.path());

        assert!(result.is_err());
    }
}
