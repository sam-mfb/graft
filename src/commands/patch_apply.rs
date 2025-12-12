use std::fs;
use std::io;
use std::path::Path;

use crate::utils::patch::apply_patch;

pub fn run(orig: &Path, patch: &Path, output: &Path) -> io::Result<()> {
    let orig_data = fs::read(orig)?;
    let patch_data = fs::read(patch)?;
    let result = apply_patch(&orig_data, &patch_data)?;
    fs::write(output, result)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::patch::create_patch;
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn applies_patch_to_file() {
        let mut orig = NamedTempFile::new().unwrap();
        let patch_file = NamedTempFile::new().unwrap();
        let output = NamedTempFile::new().unwrap();

        let orig_content = b"original content";
        let new_content = b"modified content";

        orig.write_all(orig_content).unwrap();

        let patch_data = create_patch(orig_content, new_content).unwrap();
        fs::write(patch_file.path(), &patch_data).unwrap();

        run(orig.path(), patch_file.path(), output.path()).unwrap();

        let result = fs::read(output.path()).unwrap();
        assert_eq!(result, new_content);
    }

    #[test]
    fn nonexistent_file_returns_error() {
        let patch_file = NamedTempFile::new().unwrap();
        let output = NamedTempFile::new().unwrap();
        let nonexistent = Path::new("/nonexistent/file.bin");

        let result = run(nonexistent, patch_file.path(), output.path());

        assert!(result.is_err());
    }
}
