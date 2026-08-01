use anyhow::{Context, Result};
use std::fs::{File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use super::{JournalEntry, JournalReadError, JournalReader};
use crate::core::ResolvedPaths;

#[derive(Debug)]
pub struct JournalWriter {
    path: PathBuf,
    writer: BufWriter<File>,
}

impl JournalWriter {
    pub fn open_for_package(root: &Path, package_id: &str, version: &str) -> Result<Self> {
        let package_key = crate::journal::package_journal_key(package_id, version);
        let journal_path = crate::core::package_journal_file_at(root, &package_key);

        Self::open_at(journal_path)
    }

    pub fn open_for_package_in(
        paths: &ResolvedPaths,
        package_id: &str,
        version: &str,
    ) -> Result<Self> {
        let package_key = crate::journal::package_journal_key(package_id, version);
        let journal_path = paths.package_journal_file(&package_key);

        Self::open_at(journal_path)
    }

    fn open_at(journal_path: PathBuf) -> Result<Self> {
        if journal_path.exists() {
            match JournalReader::read_committed(&journal_path) {
                Ok(_) => {
                    anyhow::bail!(
                        "journal at {} is already committed — use a new version or remove it first",
                        journal_path.display()
                    );
                }
                Err(JournalReadError::Incomplete { .. }) => {}
                Err(JournalReadError::Read { .. }) => {}
                Err(_) => {
                    anyhow::bail!(
                        "journal at {} is in an unexpected state",
                        journal_path.display()
                    );
                }
            }
        }

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&journal_path)
            .with_context(|| format!("failed to open {}", journal_path.display()))?;

        Ok(Self {
            path: journal_path,
            writer: BufWriter::new(file),
        })
    }

    pub fn append(&mut self, entry: &JournalEntry) -> Result<()> {
        serde_json::to_writer(&mut self.writer, entry)
            .context("failed to serialize journal entry")?;
        self.writer
            .write_all(b"\n")
            .context("failed to write journal entry delimiter")?;

        Ok(())
    }

    /// Flush buffered writes to the OS and fsync the file to physical disk.
    ///
    /// `BufWriter::flush` alone only hands the bytes to the OS page cache; a
    /// crash or power loss immediately after a caller sees `Ok(())` here can
    /// still lose data that was never fsynced, leaving what looks like a
    /// committed journal on disk that actually never made it there. Callers
    /// (and doctor/repair) treat a journal ending in a `Commit` entry as
    /// authoritative for rebuilding package state, so this durability step
    /// isn't optional -- it's what makes that authority claim true.
    pub fn flush(&mut self) -> Result<()> {
        self.writer
            .flush()
            .context("failed to flush journal writer")?;
        self.writer
            .get_ref()
            .sync_all()
            .context("failed to fsync journal writer")
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for JournalWriter {
    fn drop(&mut self) {
        let _ = self.writer.flush();
    }
}
