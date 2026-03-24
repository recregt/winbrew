use anyhow::{Context, Result, bail};
use sha2::{Digest, Sha256};
use std::fs::File;
use std::io::Read;
use std::path::Path;

const BUFFER_SIZE: usize = 64 * 1024;

pub fn verify_file(path: &Path, expected: &str) -> Result<()> {
    let expected = expected
        .strip_prefix("sha256:")
        .unwrap_or(expected)
        .to_ascii_lowercase();
    let file = File::open(path).context("failed to open file for checksum")?;

    let actual = match unsafe { memmap2::MmapOptions::new().map(&file) } {
        Ok(mmap) => hex::encode(Sha256::digest(&mmap)),
        Err(_) => {
            let mut hasher = Sha256::new();
            let mut reader = file;
            let mut buffer = [0u8; BUFFER_SIZE];

            loop {
                let read = reader
                    .read(&mut buffer)
                    .context("failed to read file for checksum")?;

                if read == 0 {
                    break;
                }

                hasher.update(&buffer[..read]);
            }

            hex::encode(hasher.finalize())
        }
    };

    if actual != expected {
        bail!(
            "checksum mismatch\n  expected: {}\n  actual:   {}",
            expected,
            actual
        );
    }

    Ok(())
}

pub fn seed_hasher(path: &Path, hasher: &mut Sha256) -> Result<()> {
    let file = File::open(path).context("failed to open partial download for hashing")?;

    if let Ok(mmap) = unsafe { memmap2::MmapOptions::new().map(&file) } {
        hasher.update(&mmap);
        return Ok(());
    }

    let mut reader = file;
    let mut buffer = [0u8; BUFFER_SIZE];

    loop {
        let read = reader
            .read(&mut buffer)
            .context("failed to read partial download for hashing")?;

        if read == 0 {
            break;
        }

        hasher.update(&buffer[..read]);
    }

    Ok(())
}
