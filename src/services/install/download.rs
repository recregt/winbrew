use anyhow::{Context, Result};
use std::fs;
use std::io::{BufWriter, Read, Write};
use std::path::Path;

use crate::core::cancel::check;
use crate::core::hash::{
    HashAlgorithm, HashError, Hasher, hash_algorithm, normalize_hash, verify_hash,
};
use crate::models::CatalogInstaller;

const CATALOG_USER_AGENT: &str = "winbrew-package-installer";

pub fn build_client() -> Result<reqwest::blocking::Client> {
    reqwest::blocking::Client::builder()
        .user_agent(CATALOG_USER_AGENT)
        .build()
        .context("failed to build HTTP client")
}

pub fn download_installer<FStart, FProgress>(
    client: &reqwest::blocking::Client,
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
        let mut response = client
            .get(&installer.url)
            .send()
            .with_context(|| format!("failed to request installer {}", installer.url))?
            .error_for_status()
            .context("installer request failed")?;

        check()?;

        on_start(response.content_length());

        let file = fs::File::create(&temp_path).with_context(|| {
            format!(
                "failed to create installer download file at {}",
                temp_path.display()
            )
        })?;

        let mut writer = BufWriter::with_capacity(64 * 1024, file);
        let mut buffer = [0u8; 64 * 1024];
        let mut verification = verification;

        loop {
            check()?;

            let read = response
                .read(&mut buffer)
                .context("failed to read installer response")?;
            if read == 0 {
                break;
            }

            let chunk = &buffer[..read];
            verification.update(chunk);
            writer
                .write_all(chunk)
                .context("failed to write installer to disk")?;
            on_progress(read as u64);
        }

        writer
            .flush()
            .context("failed to flush installer download file")?;

        check()?;

        verification.finish(&installer.hash)?;

        if download_path.exists() {
            fs::remove_file(download_path).with_context(|| {
                format!(
                    "failed to remove stale installer file at {}",
                    download_path.display()
                )
            })?;
        }

        fs::rename(&temp_path, download_path).with_context(|| {
            format!(
                "failed to finalize installer file: {} -> {}",
                temp_path.display(),
                download_path.display()
            )
        })?;

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
