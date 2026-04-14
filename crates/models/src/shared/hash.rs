//! Hash algorithm metadata used by catalog, install, and inventory code.
//!
//! The hash model is small on purpose: it carries the algorithm identity, the
//! display name used in user-facing messages, the expected hex length, and a
//! flag for legacy algorithms that should be treated with extra caution.

use core::str::FromStr;
use serde::{Deserialize, Serialize};
use std::fmt;

use super::error::ModelError;

/// Checksum algorithms that Winbrew recognizes in persisted model data.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HashAlgorithm {
    Md5,
    Sha1,
    #[default]
    Sha256,
    Sha512,
}

impl HashAlgorithm {
    /// Return the lowercase storage form used in snapshots and raw payloads.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
            Self::Sha512 => "sha512",
        }
    }

    /// Return the canonical display name used in diagnostics and CLI output.
    pub fn display_name(self) -> &'static str {
        match self {
            Self::Md5 => "MD5",
            Self::Sha1 => "SHA1",
            Self::Sha256 => "SHA256",
            Self::Sha512 => "SHA512",
        }
    }

    /// Return the expected lowercase hex length for the algorithm.
    pub fn expected_len(self) -> usize {
        match self {
            Self::Md5 => 32,
            Self::Sha1 => 40,
            Self::Sha256 => 64,
            Self::Sha512 => 128,
        }
    }

    /// Return `true` when the algorithm should be treated as legacy.
    pub fn is_legacy(self) -> bool {
        matches!(self, Self::Md5 | Self::Sha1)
    }

    /// Detect the checksum algorithm encoded in a hash string.
    pub fn detect(value: &str) -> Option<Self> {
        let trimmed = value.trim();
        if trimmed.is_empty() {
            return None;
        }

        let lower = trimmed.to_ascii_lowercase();

        for (prefix, algorithm) in [
            ("sha512:", Self::Sha512),
            ("sha256:", Self::Sha256),
            ("sha1:", Self::Sha1),
            ("md5:", Self::Md5),
        ] {
            if lower.starts_with(prefix) {
                return Some(algorithm);
            }
        }

        let candidate = lower
            .strip_prefix("sha512:")
            .or_else(|| lower.strip_prefix("sha256:"))
            .or_else(|| lower.strip_prefix("sha1:"))
            .or_else(|| lower.strip_prefix("md5:"))
            .unwrap_or(lower.as_str());

        [Self::Sha512, Self::Sha256, Self::Sha1, Self::Md5]
            .into_iter()
            .find(|algorithm| candidate.len() == algorithm.expected_len())
    }
}

impl fmt::Display for HashAlgorithm {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.display_name())
    }
}

impl FromStr for HashAlgorithm {
    type Err = ModelError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value.trim().to_ascii_lowercase().as_str() {
            "md5" => Ok(Self::Md5),
            "sha1" => Ok(Self::Sha1),
            "sha256" => Ok(Self::Sha256),
            "sha512" => Ok(Self::Sha512),
            other => Err(ModelError::invalid_enum_value("hash.algorithm", other)),
        }
    }
}
