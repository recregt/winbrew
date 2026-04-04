use anyhow::{Result, bail};
use sha2::{Digest, Sha256};

pub fn sha256_hasher() -> Sha256 {
    Sha256::new()
}

pub fn sha256_update(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update(bytes);
}

pub fn sha256_finalize(hasher: Sha256) -> [u8; 32] {
    let digest = hasher.finalize();
    let mut bytes = [0u8; 32];
    bytes.copy_from_slice(&digest);
    bytes
}

pub fn verify_hash(expected_hash: &str, actual_hash: impl AsRef<[u8]>) -> Result<()> {
    let expected_hash = normalize_hash(expected_hash);
    if expected_hash.is_empty() {
        return Ok(());
    }

    let bytes = actual_hash.as_ref();
    let mut actual_hash = String::with_capacity(bytes.len() * 2);
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    for &byte in bytes {
        actual_hash.push(HEX_CHARS[(byte >> 4) as usize] as char);
        actual_hash.push(HEX_CHARS[(byte & 0x0f) as usize] as char);
    }

    if actual_hash != expected_hash {
        bail!("checksum mismatch for installer: expected {expected_hash}, got {actual_hash}");
    }

    Ok(())
}

pub fn normalize_hash(value: &str) -> String {
    let trimmed = value.trim();
    trimmed
        .strip_prefix("sha256:")
        .unwrap_or(trimmed)
        .to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{normalize_hash, verify_hash};

    #[test]
    fn normalize_hash_strips_prefix_and_whitespace() {
        assert_eq!(normalize_hash("  sha256:ABC123  "), "abc123");
        assert_eq!(normalize_hash(" ABC123  "), "abc123");
    }

    #[test]
    fn verify_hash_accepts_matching_hash() {
        let actual = [0x12, 0x34, 0xab, 0xcd];
        assert!(verify_hash("sha256:1234abcd", actual).is_ok());
    }

    #[test]
    fn verify_hash_rejects_mismatch() {
        let actual = [0x12, 0x34, 0xab, 0xcd];
        assert!(verify_hash("sha256:11111111", actual).is_err());
    }
}
