//! Download and verification helpers for installer payloads.
//!
//! This module owns the network-specific half of the install flow. It creates
//! the dedicated installer HTTP client, streams the selected installer into a
//! temporary file, and finalizes the file only after checksum verification has
//! passed.
//!
//! The higher-level orchestration code uses these helpers as a single phase with
//! well-defined cleanup behavior: temporary files are removed on failure and the
//! caller receives any tolerated legacy checksum algorithms for reporting.

use anyhow::Result;
use std::path::Path;

use crate::core::cancel::check;
use crate::core::fs::{cleanup_path, finalize_temp_file};
use crate::core::hash::{Hasher, verify_hash};
use crate::core::network::{build_client as network_build_client, download_url_to_temp_file};
use crate::models::catalog::CatalogInstaller;
use crate::models::domains::shared::HashAlgorithm;

const CATALOG_USER_AGENT: &str = "winbrew-package-installer";

/// Build the HTTP client used for installer downloads.
///
/// A dedicated user agent makes installer traffic easy to identify in server
/// logs and keeps the install pipeline separate from catalog refresh traffic.
pub fn build_client() -> Result<crate::core::network::Client> {
    Ok(network_build_client(CATALOG_USER_AGENT)?)
}

/// Download an installer into a temporary file and verify it before finalizing.
///
/// The payload is streamed to a `.part` file next to `download_path`, with
/// progress forwarded through the provided callbacks. If the installer hash is
/// present, it is verified as the bytes arrive. On success, the temporary file
/// is atomically finalized into `download_path` and the set of tolerated legacy
/// checksum algorithms is returned to the caller.
///
/// When any step fails, the temporary file is removed so the install flow does
/// not leave behind partially downloaded payloads.
pub fn download_installer<FStart, FProgress>(
    client: &crate::core::network::Client,
    installer: &CatalogInstaller,
    download_path: &Path,
    ignore_checksum_security: bool,
    on_start: FStart,
    mut on_progress: FProgress,
) -> Result<Vec<HashAlgorithm>>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let temp_path = download_path.with_extension("part");
    let download_result = (|| -> Result<Vec<HashAlgorithm>> {
        check()?;

        let (verification, legacy_checksum_algorithms) = verify_strategy(
            &installer.hash,
            installer.hash_algorithm,
            ignore_checksum_security,
        )?;
        let mut verification = verification;

        check()?;

        download_url_to_temp_file(
            client,
            &installer.url,
            &temp_path,
            "installer",
            on_start,
            &mut on_progress,
            |chunk| {
                check()?;
                verification.update(chunk);
                Ok(())
            },
        )?;

        verification.finish(&installer.hash)?;

        finalize_temp_file(&temp_path, download_path)?;

        Ok(legacy_checksum_algorithms)
    })();

    if download_result.is_err() {
        let _ = cleanup_path(&temp_path);
    }

    download_result
}

enum Verification {
    None,
    Active(Box<Hasher>),
}

impl Verification {
    fn update(&mut self, chunk: &[u8]) {
        match self {
            Self::None => {}
            Self::Active(hasher) => hasher.update(chunk),
        }
    }

    fn finish(self, expected_hash: &str) -> Result<()> {
        match self {
            Self::None => Ok(()),
            Self::Active(hasher) => {
                verify_hash(expected_hash, hasher.finalize()).map_err(Into::into)
            }
        }
    }
}

fn verify_strategy(
    expected_hash: &str,
    hash_algorithm: HashAlgorithm,
    ignore_checksum_security: bool,
) -> Result<(Verification, Vec<HashAlgorithm>)> {
    let trimmed = expected_hash.trim();

    if trimmed.is_empty() {
        return Ok((Verification::None, Vec::new()));
    }

    match hash_algorithm {
        // MD5 is cryptographically broken beyond use; skip verification entirely
        // rather than computing a hash we cannot trust.
        HashAlgorithm::Md5 if ignore_checksum_security => {
            Ok((Verification::None, vec![HashAlgorithm::Md5]))
        }
        HashAlgorithm::Md5 => Err(crate::core::HashError::LegacyChecksumAlgorithm {
            algorithm: HashAlgorithm::Md5,
        }
        .into()),
        HashAlgorithm::Sha1 if ignore_checksum_security => Ok((
            Verification::Active(Box::new(Hasher::new(HashAlgorithm::Sha1))),
            vec![HashAlgorithm::Sha1],
        )),
        HashAlgorithm::Sha1 => Err(crate::core::HashError::LegacyChecksumAlgorithm {
            algorithm: HashAlgorithm::Sha1,
        }
        .into()),
        algorithm => Ok((
            Verification::Active(Box::new(Hasher::new(algorithm))),
            Vec::new(),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::{Verification, verify_strategy};
    use crate::core::HashError;
    use crate::models::domains::shared::HashAlgorithm;

    #[test]
    fn verify_strategy_rejects_md5_without_ignore_flag() {
        let err = match verify_strategy("abc123", HashAlgorithm::Md5, false) {
            Ok(_) => panic!("md5 should be rejected by default"),
            Err(err) => err,
        };

        assert!(matches!(
            err.downcast_ref::<HashError>(),
            Some(HashError::LegacyChecksumAlgorithm {
                algorithm: HashAlgorithm::Md5
            })
        ));
    }

    #[test]
    fn verify_strategy_tolerates_md5_with_ignore_flag() {
        let (verification, legacy_checksum_algorithms) =
            verify_strategy("abc123", HashAlgorithm::Md5, true)
                .expect("md5 should be tolerated when ignoring checksum security");

        assert!(matches!(verification, Verification::None));
        assert_eq!(legacy_checksum_algorithms, vec![HashAlgorithm::Md5]);
    }

    #[test]
    fn verify_strategy_skips_verification_for_empty_hash() {
        let (verification, legacy_checksum_algorithms) =
            verify_strategy("   ", HashAlgorithm::Sha256, false)
                .expect("empty hashes should bypass verification");

        assert!(matches!(verification, Verification::None));
        assert!(legacy_checksum_algorithms.is_empty());
    }

    #[test]
    fn verify_strategy_still_verifies_sha1_when_ignored() {
        let (verification, legacy_checksum_algorithms) =
            verify_strategy("abc123", HashAlgorithm::Sha1, true)
                .expect("sha1 should remain verifiable when security checks are ignored");

        assert!(matches!(verification, Verification::Active(_)));
        assert_eq!(legacy_checksum_algorithms, vec![HashAlgorithm::Sha1]);
    }
}
