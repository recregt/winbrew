use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::core::cancel::check;
use crate::core::fs::finalize_temp_file;
use crate::core::hash::{
    HashAlgorithm, HashError, Hasher, hash_algorithm, normalize_hash, verify_hash,
};
use crate::core::network::{build_client as network_build_client, download_url_to_temp_file};
use crate::models::CatalogInstaller;

const CATALOG_USER_AGENT: &str = "winbrew-package-installer";

pub fn build_client() -> Result<crate::core::network::Client> {
    network_build_client(CATALOG_USER_AGENT)
}

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
    let result = (|| -> Result<Vec<HashAlgorithm>> {
        check()?;

        let (verification, legacy_checksum_algorithms) =
            verify_strategy(&installer.hash, ignore_checksum_security)?;
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

    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }

    result
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
    ignore_checksum_security: bool,
) -> Result<(Verification, Vec<HashAlgorithm>)> {
    let trimmed = expected_hash.trim();

    if trimmed.is_empty() {
        return Ok((Verification::None, Vec::new()));
    }

    if normalize_hash(trimmed).is_empty() {
        return Ok((Verification::None, Vec::new()));
    }

    match hash_algorithm(trimmed) {
        Some(HashAlgorithm::Md5) if ignore_checksum_security => {
            Ok((Verification::None, vec![HashAlgorithm::Md5]))
        }
        Some(HashAlgorithm::Md5) => Err(HashError::LegacyChecksumAlgorithm {
            algorithm: HashAlgorithm::Md5,
        }
        .into()),
        Some(HashAlgorithm::Sha1) if ignore_checksum_security => Ok((
            Verification::Active(Box::new(Hasher::new(HashAlgorithm::Sha1))),
            vec![HashAlgorithm::Sha1],
        )),
        Some(HashAlgorithm::Sha1) => Err(HashError::LegacyChecksumAlgorithm {
            algorithm: HashAlgorithm::Sha1,
        }
        .into()),
        Some(algorithm) => Ok((
            Verification::Active(Box::new(Hasher::new(algorithm))),
            Vec::new(),
        )),
        None => anyhow::bail!("unsupported checksum format for installer: {expected_hash}"),
    }
}
