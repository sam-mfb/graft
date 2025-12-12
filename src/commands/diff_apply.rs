use std::fs;
use std::io;
use std::path::Path;

use crate::utils::diff::apply_diff;

pub fn run(orig: &Path, diff: &Path, output: &Path) -> io::Result<()> {
    let orig_data = fs::read(orig)?;
    let diff_data = fs::read(diff)?;
    let result = apply_diff(&orig_data, &diff_data)?;
    fs::write(output, result)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::diff::create_diff;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn applies_diff_to_file() {
        let mut orig = NamedTempFile::new().unwrap();
        let diff_file = NamedTempFile::new().unwrap();
        let output = NamedTempFile::new().unwrap();

        let orig_content = b"original content";
        let new_content = b"modified content";

        orig.write_all(orig_content).unwrap();

        let diff_data = create_diff(orig_content, new_content).unwrap();
        fs::write(diff_file.path(), &diff_data).unwrap();

        run(orig.path(), diff_file.path(), output.path()).unwrap();

        let result = fs::read(output.path()).unwrap();
        assert_eq!(result, new_content);
    }

    #[test]
    fn nonexistent_file_returns_error() {
        let diff_file = NamedTempFile::new().unwrap();
        let output = NamedTempFile::new().unwrap();
        let nonexistent = Path::new("/nonexistent/file.bin");

        let result = run(nonexistent, diff_file.path(), output.path());

        assert!(result.is_err());
    }
}
