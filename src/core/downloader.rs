use anyhow::{Context, Result, bail};
use reqwest::StatusCode;
use sha2::{Digest, Sha256};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};

const BUFFER_SIZE: usize = 64 * 1024;

pub fn download<F>(url: &str, dest: &Path, mut on_progress: F) -> Result<()>
where
    F: FnMut(u64, u64),
{
    let temp_dest = dest.with_extension("part");
    let mut temp_guard = TempFileGuard {
        path: temp_dest.clone(),
        keep: false,
    };

    let requested_existing_size = if temp_dest.exists() {
        fs::metadata(&temp_dest).map(|m| m.len()).unwrap_or(0)
    } else {
        0
    };

    let client = reqwest::blocking::Client::new();
    let mut request = client.get(url);

    if requested_existing_size > 0 {
        request = request.header("Range", format!("bytes={}-", requested_existing_size));
    }

    let mut response = request
        .send()
        .context("failed to connect")?
        .error_for_status()
        .context("server returned error")?;

    let resuming = requested_existing_size > 0 && response.status() == StatusCode::PARTIAL_CONTENT;
    let existing_size = if resuming { requested_existing_size } else { 0 };

    let total_size = response.content_length().unwrap_or(0) + existing_size;

    let file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(resuming)
        .truncate(!resuming)
        .open(&temp_dest)
        .context("failed to open destination file")?;

    if total_size > 0 {
        file.set_len(total_size)
            .context("failed to pre-allocate destination file")?;
    }

    let mut writer = BufWriter::with_capacity(BUFFER_SIZE, file);
    let mut buffer = [0u8; BUFFER_SIZE];
    let mut downloaded = existing_size;

    if downloaded > 0 {
        on_progress(downloaded, total_size);
    }

    loop {
        let read = response
            .read(&mut buffer)
            .context("failed to read response")?;

        if read == 0 {
            break;
        }

        writer
            .write_all(&buffer[..read])
            .context("failed to write file")?;

        downloaded += read as u64;
        on_progress(downloaded, total_size);
    }

    writer.flush().context("failed to flush buffer")?;

    if total_size > 0 && downloaded != total_size {
        bail!(
            "incomplete download: expected {} bytes, got {}",
            total_size,
            downloaded
        );
    }

    if dest.exists() {
        fs::remove_file(dest).context("failed to replace existing destination file")?;
    }

    fs::rename(&temp_dest, dest).context("failed to finalize downloaded file")?;
    temp_guard.keep = true;

    Ok(())
}

pub fn verify_checksum(path: &Path, expected: &str) -> Result<()> {
    let expected = expected.strip_prefix("sha256:").unwrap_or(expected);

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

pub fn download_and_verify<F>(url: &str, dest: &Path, checksum: &str, on_progress: F) -> Result<()>
where
    F: FnMut(u64, u64),
{
    download(url, dest, on_progress)?;

    if let Err(err) = verify_checksum(dest, checksum) {
        let _ = fs::remove_file(dest);
        return Err(err);
    }

    Ok(())
}

struct TempFileGuard {
    path: PathBuf,
    keep: bool,
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}
