use std::io;

pub fn create_patch(old: &[u8], new: &[u8]) -> io::Result<Vec<u8>> {
    let mut patch = Vec::new();
    bsdiff::diff(old, new, &mut patch)?;
    Ok(patch)
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

        let mut restored = Vec::new();
        bsdiff::patch(old, &mut patch.as_slice(), &mut restored).unwrap();

        assert_eq!(restored, new);
    }
}
