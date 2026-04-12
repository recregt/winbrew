use anyhow::{Context, Result};
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use super::{JournalEntry, JournalReadError, JournalReader};

#[derive(Debug)]
pub struct JournalWriter {
    path: PathBuf,
    writer: BufWriter<File>,
}

impl JournalWriter {
    pub fn open_for_package(root: &Path, package_id: &str, version: &str) -> Result<Self> {
        let package_key = winbrew_core::package_journal_key(package_id, version);
        let journal_path = winbrew_core::package_journal_file_at(root, &package_key);

        if let Some(parent) = journal_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }

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

    pub fn flush(&mut self) -> Result<()> {
        self.writer
            .flush()
            .context("failed to flush journal writer")
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
