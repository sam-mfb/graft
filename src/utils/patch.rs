use std::io;

pub fn create_patch(old: &[u8], new: &[u8]) -> io::Result<Vec<u8>> {
    let mut patch = Vec::new();
    bsdiff::diff(old, new, &mut patch)?;
    Ok(patch)
}

pub fn apply_patch(orig: &[u8], patch: &[u8]) -> io::Result<Vec<u8>> {
    let mut output = Vec::new();
    bsdiff::patch(orig, &mut patch.as_ref(), &mut output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_patch_for_different_data() {
        let old = b"hello world";
        let new = b"hello rust";

        let patch = create_patch(old, new).unwrap();

        assert!(!patch.is_empty());
    }

    #[test]
    fn creates_patch_for_identical_data() {
        let data = b"same content";

        let patch = create_patch(data, data).unwrap();

        assert!(!patch.is_empty());
    }

    #[test]
    fn patch_can_be_applied() {
        let old = b"original file content";
        let new = b"modified file content here";

        let patch = create_patch(old, new).unwrap();
        let restored = apply_patch(old, &patch).unwrap();

        assert_eq!(restored, new);
    }

    #[test]
    fn apply_patch_roundtrip() {
        let orig = b"the quick brown fox";
        let modified = b"the slow brown dog";

        let patch = create_patch(orig, modified).unwrap();
        let result = apply_patch(orig, &patch).unwrap();

        assert_eq!(result, modified);
    }
}
