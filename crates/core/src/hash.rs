//! Hashing primitives and checksum policy for download and inventory flows.
//!
//! This module keeps the algorithm mapping, streaming hash wrapper, and
//! checksum error vocabulary together so callers can validate installer and
//! catalog payloads without re-implementing algorithm handling.

use md5::Md5;
use sha1::Sha1;
use sha2::{Digest, Sha256, Sha512};
use std::fs::File;
use std::io::{self, Write};
use std::path::Path;
use thiserror::Error;
use winbrew_models::shared::hash::HashAlgorithm;

/// Errors produced while calculating or validating checksums.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum HashError {
    #[error("checksum mismatch for installer: expected {expected}, got {actual}")]
    ChecksumMismatch { expected: String, actual: String },

    #[error("{algorithm} checksums are disabled by default for security")]
    LegacyChecksumAlgorithm { algorithm: HashAlgorithm },

    #[error("no expected checksum was provided for verification")]
    MissingExpectedHash,
}

pub type Result<T> = std::result::Result<T, HashError>;

/// Streaming hash wrapper over the supported checksum algorithms.
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

/// Detect the checksum algorithm referenced by a hash string.
pub fn hash_algorithm(value: &str) -> Option<HashAlgorithm> {
    HashAlgorithm::detect(value)
}

/// Verify `actual_hash` against `expected_hash`.
///
/// A blank `expected_hash` (empty, whitespace-only, or just an algorithm
/// prefix) is an error, not an automatic pass -- callers that intend to skip
/// verification when no hash is available must decide that explicitly before
/// calling this function, the way `verify_strategy` in the install download
/// path does. Treating "nothing to compare against" as "verified" here would
/// let a blank hash field anywhere upstream (a malformed catalog entry, a
/// missing inventory snapshot value) silently defeat every caller's
/// integrity check at once.
pub fn verify_hash(expected_hash: &str, actual_hash: impl AsRef<[u8]>) -> Result<()> {
    let expected_hash = normalize_hash(expected_hash);
    if expected_hash.is_empty() {
        return Err(HashError::MissingExpectedHash);
    }

    let bytes = actual_hash.as_ref();
    let mut actual_hash = String::with_capacity(bytes.len() * 2);
    const HEX_CHARS: &[u8; 16] = b"0123456789abcdef";

    for &byte in bytes {
        actual_hash.push(HEX_CHARS[(byte >> 4) as usize] as char);
        actual_hash.push(HEX_CHARS[(byte & 0x0f) as usize] as char);
    }

    if actual_hash != expected_hash {
        return Err(HashError::ChecksumMismatch {
            expected: expected_hash,
            actual: actual_hash,
        });
    }

    Ok(())
}

/// Verifies `actual_hash` against `expected_hash`, rejecting legacy (MD5/SHA-1)
/// algorithms unless the caller explicitly opts in via `allow_legacy`.
///
/// `verify_hash` alone is a pure byte comparison with no opinion about
/// algorithm strength -- it does not enforce the "legacy checksums are
/// rejected by default" guarantee the project advertises. That enforcement
/// used to live only in the install download path (`verify_strategy` in
/// `winbrew-app`), which meant any other caller of `verify_hash` got no
/// legacy-algorithm protection at all. Route verification through this
/// function instead so the guarantee holds everywhere a hash is checked, not
/// just wherever a caller happened to replicate the policy.
pub fn verify_hash_with_policy(
    algorithm: HashAlgorithm,
    expected_hash: &str,
    actual_hash: impl AsRef<[u8]>,
    allow_legacy: bool,
) -> Result<()> {
    if algorithm.is_legacy() && !allow_legacy {
        return Err(HashError::LegacyChecksumAlgorithm { algorithm });
    }

    verify_hash(expected_hash, actual_hash)
}

pub fn hash_file(path: &Path, algorithm: HashAlgorithm) -> io::Result<Vec<u8>> {
    let mut file = File::open(path)?;
    let mut writer = HashWriter::new(Hasher::new(algorithm));

    io::copy(&mut file, &mut writer)?;

    Ok(writer.finish())
}

pub fn normalize_hash(value: &str) -> String {
    let trimmed = value.trim();
    let stripped = ["md5:", "sha1:", "sha256:", "sha512:"]
        .into_iter()
        .find_map(|prefix| trimmed.strip_prefix(prefix))
        .unwrap_or(trimmed);

    stripped.to_ascii_lowercase()
}

struct HashWriter {
    hasher: Hasher,
}

impl HashWriter {
    fn new(hasher: Hasher) -> Self {
        Self { hasher }
    }

    fn finish(self) -> Vec<u8> {
        self.hasher.finalize()
    }
}

impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.hasher.update(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        HashAlgorithm, HashError, Hasher, hash_algorithm, hash_file, normalize_hash, verify_hash,
        verify_hash_with_policy,
    };
    use sha2::{Digest, Sha256, Sha512};
    use std::fs;
    use tempfile::tempdir;

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
    fn verify_hash_rejects_blank_expected_hash() {
        let actual = [0x12, 0x34, 0xab, 0xcd];

        assert_eq!(
            verify_hash("", actual),
            Err(super::HashError::MissingExpectedHash)
        );
        assert_eq!(
            verify_hash("   ", actual),
            Err(super::HashError::MissingExpectedHash)
        );
        assert_eq!(
            verify_hash("sha256:", actual),
            Err(super::HashError::MissingExpectedHash)
        );
    }

    #[test]
    fn verify_hash_with_policy_rejects_legacy_algorithms_by_default() {
        let actual = [0x12, 0x34, 0xab, 0xcd];

        for algorithm in [HashAlgorithm::Md5, HashAlgorithm::Sha1] {
            assert_eq!(
                verify_hash_with_policy(algorithm, "sha256:1234abcd", actual, false),
                Err(HashError::LegacyChecksumAlgorithm { algorithm })
            );
        }
    }

    #[test]
    fn verify_hash_with_policy_allows_legacy_algorithms_when_opted_in() {
        let actual = [0x12, 0x34, 0xab, 0xcd];

        assert!(
            verify_hash_with_policy(HashAlgorithm::Md5, "sha256:1234abcd", actual, true).is_ok()
        );
        assert!(
            verify_hash_with_policy(HashAlgorithm::Sha1, "sha256:1234abcd", actual, true).is_ok()
        );
    }

    #[test]
    fn verify_hash_with_policy_never_restricts_modern_algorithms() {
        let actual = [0x12, 0x34, 0xab, 0xcd];

        assert!(
            verify_hash_with_policy(HashAlgorithm::Sha256, "sha256:1234abcd", actual, false)
                .is_ok()
        );
        assert_eq!(
            verify_hash_with_policy(HashAlgorithm::Sha256, "sha256:11111111", actual, false),
            Err(HashError::ChecksumMismatch {
                expected: "11111111".to_string(),
                actual: "1234abcd".to_string(),
            })
        );
    }

    #[test]
    fn hasher_streams_sha512_chunks() {
        let mut hasher = Hasher::new(HashAlgorithm::Sha512);
        hasher.update(b"ab");
        hasher.update(b"c");

        assert_eq!(hasher.finalize(), Sha512::digest(b"abc").to_vec());
    }

    #[test]
    fn hash_file_streams_contents() {
        let temp_dir = tempdir().expect("temp dir");
        let path = temp_dir.path().join("payload.bin");

        fs::write(&path, b"abc").expect("write payload");

        let digest = hash_file(&path, HashAlgorithm::Sha256).expect("hash file");

        assert_eq!(digest, Sha256::digest(b"abc").to_vec());
    }
}
