use std::io;

pub fn create_diff(old: &[u8], new: &[u8]) -> io::Result<Vec<u8>> {
    let mut diff = Vec::new();
    bsdiff::diff(old, new, &mut diff)?;
    Ok(diff)
}

pub fn apply_diff(orig: &[u8], diff: &[u8]) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    bsdiff::patch(orig, &mut diff.as_ref(), &mut output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_diff_for_different_data() {
        let old = b"hello world";
        let new = b"hello rust";

        let diff = create_diff(old, new).unwrap();

        assert!(!diff.is_empty());
    }

    #[test]
    fn creates_diff_for_identical_data() {
        let data = b"same content";

        let diff = create_diff(data, data).unwrap();

        assert!(!diff.is_empty());
    }

    #[test]
    fn diff_can_be_applied() {
        let old = b"original file content";
        let new = b"modified file content here";

        let diff = create_diff(old, new).unwrap();
        let restored = apply_diff(old, &diff).unwrap();

        assert_eq!(restored, new);
    }

    #[test]
    fn apply_diff_roundtrip() {
        let orig = b"the quick brown fox";
        let modified = b"the slow brown dog";

        let diff = create_diff(orig, modified).unwrap();
        let result = apply_diff(orig, &diff).unwrap();

        assert_eq!(result, modified);
    }
}
