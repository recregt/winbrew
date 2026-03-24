use anyhow::{Context, Result};
use reqwest::StatusCode;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

#[cfg(windows)]
use std::os::windows::fs::OpenOptionsExt;

const BUFFER_SIZE: usize = 64 * 1024;

pub struct TempFileGuard {
    path: PathBuf,
    keep: bool,
}

impl TempFileGuard {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            keep: false,
        }
    }

    pub fn keep(&mut self) {
        self.keep = true;
    }
}

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        if !self.keep {
            let _ = fs::remove_file(&self.path);
        }
    }
}

pub struct DownloadTarget {
    pub(crate) writer: BufWriter<File>,
    pub(crate) temp_path: PathBuf,
    pub(crate) existing_size: u64,
    pub(crate) total_size: u64,
}

impl DownloadTarget {
    pub fn new(
        dest: &Path,
        response: &reqwest::blocking::Response,
        requested_existing_size: u64,
    ) -> Result<Self> {
        let temp_path = dest.with_extension("part");

        let resuming =
            requested_existing_size > 0 && response.status() == StatusCode::PARTIAL_CONTENT;
        let existing_size = if resuming { requested_existing_size } else { 0 };
        let total_size = response.content_length().unwrap_or(0) + existing_size;

        let mut options = OpenOptions::new();
        options
            .create(true)
            .write(true)
            .append(resuming)
            .truncate(!resuming);

        #[cfg(windows)]
        {
            options.share_mode(1);
        }

        let file = options
            .open(&temp_path)
            .context("failed to open destination file")?;

        if total_size > 0 {
            file.set_len(total_size)
                .context("failed to pre-allocate destination file")?;
        }

        Ok(Self {
            writer: BufWriter::with_capacity(BUFFER_SIZE, file),
            temp_path,
            existing_size,
            total_size,
        })
    }

    pub fn finalize(mut self, dest: &Path) -> Result<()> {
        self.writer.flush().context("failed to flush buffer")?;

        if dest.exists() {
            fs::remove_file(dest).context("failed to replace existing destination file")?;
        }

        fs::rename(&self.temp_path, dest).context("failed to finalize downloaded file")?;
        Ok(())
    }
}
