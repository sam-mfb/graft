use sha2::{Digest, Sha256};

pub fn hash_bytes(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    let result = hasher.finalize();
    format!("{:x}", result)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hashes_bytes() {
        let hash = hash_bytes(b"hello world");
        assert_eq!(hash.len(), 64); // SHA-256 produces 64 hex chars
    }

    #[test]
    fn same_input_same_hash() {
        let hash1 = hash_bytes(b"test data");
        let hash2 = hash_bytes(b"test data");
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn different_input_different_hash() {
        let hash1 = hash_bytes(b"data a");
        let hash2 = hash_bytes(b"data b");
        assert_ne!(hash1, hash2);
    }
}
