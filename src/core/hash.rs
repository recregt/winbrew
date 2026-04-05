use anyhow::{Result, bail};
use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HashAlgorithm {
    Md5,
    Sha1,
    Sha256,
    Sha512,
}

impl HashAlgorithm {
    pub fn expected_len(self) -> usize {
        match self {
            Self::Md5 => 32,
            Self::Sha1 => 40,
            Self::Sha256 => 64,
            Self::Sha512 => 128,
        }
    }

    pub fn is_legacy(self) -> bool {
        matches!(self, Self::Md5 | Self::Sha1)
    }
}

#[derive(Debug)]
pub enum Hasher {
    Md5(Md5),
    Sha1(Sha1),
    Sha256(Sha256),
    Sha512(Sha512),
}

impl Hasher {
    pub fn new(algorithm: HashAlgorithm) -> Self {
        match algorithm {
            HashAlgorithm::Md5 => Self::Md5(Md5::new()),
            HashAlgorithm::Sha1 => Self::Sha1(Sha1::new()),
            HashAlgorithm::Sha256 => Self::Sha256(Sha256::new()),
            HashAlgorithm::Sha512 => Self::Sha512(Sha512::new()),
        }
    }

    pub fn update(&mut self, bytes: &[u8]) {
        match self {
            Self::Md5(hasher) => hasher.update(bytes),
            Self::Sha1(hasher) => hasher.update(bytes),
            Self::Sha256(hasher) => hasher.update(bytes),
            Self::Sha512(hasher) => hasher.update(bytes),
        }
    }

    pub fn finalize(self) -> Vec<u8> {
        match self {
            Self::Md5(hasher) => hasher.finalize().to_vec(),
            Self::Sha1(hasher) => hasher.finalize().to_vec(),
            Self::Sha256(hasher) => hasher.finalize().to_vec(),
            Self::Sha512(hasher) => hasher.finalize().to_vec(),
        }
    }
}

pub fn hash_algorithm(value: &str) -> Option<HashAlgorithm> {
    let normalized = normalize_hash(value);

    if normalized.is_empty() {
        return None;
    }

    let trimmed = value.trim_start().to_ascii_lowercase();

    for (prefix, algorithm) in [
        ("sha512:", HashAlgorithm::Sha512),
        ("sha256:", HashAlgorithm::Sha256),
        ("sha1:", HashAlgorithm::Sha1),
        ("md5:", HashAlgorithm::Md5),
    ] {
        if trimmed.starts_with(prefix) {
            return Some(algorithm);
        }
    }

    [
        HashAlgorithm::Sha512,
        HashAlgorithm::Sha256,
        HashAlgorithm::Sha1,
        HashAlgorithm::Md5,
    ]
    .into_iter()
    .find(|algorithm| normalized.len() == algorithm.expected_len())
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
    let stripped = ["md5:", "sha1:", "sha256:", "sha512:"]
        .into_iter()
        .find_map(|prefix| trimmed.strip_prefix(prefix))
        .unwrap_or(trimmed);

    stripped.to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::{HashAlgorithm, Hasher, hash_algorithm, normalize_hash, verify_hash};
    use sha2::{Digest, Sha512};

    #[test]
    fn normalize_hash_strips_prefix_and_whitespace() {
        assert_eq!(normalize_hash("  md5:ABC123  "), "abc123");
        assert_eq!(normalize_hash("  sha256:ABC123  "), "abc123");
        assert_eq!(normalize_hash("  sha1:ABC123  "), "abc123");
        assert_eq!(normalize_hash("  sha512:ABC123  "), "abc123");
        assert_eq!(normalize_hash(" ABC123  "), "abc123");
    }

    #[test]
    fn verify_hash_accepts_matching_hash() {
        let actual = [0x12, 0x34, 0xab, 0xcd];
        assert!(verify_hash("sha256:1234abcd", actual).is_ok());
    }

    #[test]
    fn hash_algorithm_detects_supported_algorithms() {
        assert_eq!(
            hash_algorithm("md5:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            Some(HashAlgorithm::Md5)
        );
        assert_eq!(
            hash_algorithm("sha1:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"),
            Some(HashAlgorithm::Sha1)
        );
        assert_eq!(
            hash_algorithm(
                "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            ),
            Some(HashAlgorithm::Sha256)
        );
        assert_eq!(
            hash_algorithm(
                "sha512:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            ),
            Some(HashAlgorithm::Sha512)
        );
    }

    #[test]
    fn verify_hash_rejects_mismatch() {
        let actual = [0x12, 0x34, 0xab, 0xcd];
        assert!(verify_hash("sha256:11111111", actual).is_err());
    }

    #[test]
    fn hasher_streams_sha512_chunks() {
        let mut hasher = Hasher::new(HashAlgorithm::Sha512);
        hasher.update(b"ab");
        hasher.update(b"c");

        assert_eq!(hasher.finalize(), Sha512::digest(b"abc").to_vec());
    }
}
