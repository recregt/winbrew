use anyhow::{Context, Result};
use std::fs::{File, OpenOptions, TryLockError};
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

        // `.read(true)` is required for `try_lock()` to succeed on Windows:
        // a handle opened with only `.append(true)` gets FILE_APPEND_DATA
        // access, which LockFileEx rejects with ERROR_ACCESS_DENIED even
        // when the file is otherwise uncontested. Adding read access
        // doesn't change the write semantics -- appends still land at EOF
        // -- it only grants the access rights locking needs.
        let file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&journal_path)
            .with_context(|| format!("failed to open {}", journal_path.display()))?;

        // Two processes racing to install the same package (or a repair
        // replay racing an in-progress install) could otherwise both hold
        // an open handle to this file and interleave appended lines. That
        // corrupts more than the racing writer's own entries: JournalReader
        // rejects any entry appended after a Commit line as malformed,
        // meaning a second writer's interleaved bytes can retroactively
        // invalidate a first writer's already-committed, otherwise-valid
        // journal for recovery purposes. An OS-level advisory lock, held for
        // the writer's lifetime, serializes access instead: a second opener
        // fails fast rather than silently interleaving.
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                anyhow::bail!(
                    "journal at {} is already being written by another process",
                    journal_path.display()
                );
            }
            Err(TryLockError::Error(err)) => {
                return Err(err)
                    .with_context(|| format!("failed to lock {}", journal_path.display()));
            }
        }

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

#[cfg(test)]
mod tests {
    use super::JournalWriter;
    use std::path::Path;
    use tempfile::tempdir;

    fn prepare_journal_dir(root: &Path, package_id: &str, version: &str) {
        let package_key = crate::journal::package_journal_key(package_id, version);
        let journal_path = crate::core::package_journal_file_at(root, &package_key);
        std::fs::create_dir_all(
            journal_path
                .parent()
                .expect("journal path should have a parent"),
        )
        .expect("journal directory should be created");
    }

    /// Locks in the fix for the race where two processes (or a repair
    /// replay racing an in-progress install) both hold an open handle to
    /// the same journal file and interleave appended lines, which
    /// JournalReader's post-Commit trailing-entry check would then treat as
    /// corruption. Advisory locks are per open-file-description, so two
    /// independent opens from the same test process reproduce the same
    /// conflict a second real process would hit.
    #[test]
    fn open_for_package_rejects_a_second_concurrent_writer() {
        let temp_root = tempdir().expect("temp root");
        prepare_journal_dir(temp_root.path(), "Contoso.App", "1.0.0");

        let _first = JournalWriter::open_for_package(temp_root.path(), "Contoso.App", "1.0.0")
            .expect("first writer should open the journal");

        let second = JournalWriter::open_for_package(temp_root.path(), "Contoso.App", "1.0.0");

        assert!(
            second.is_err(),
            "a second writer must not be able to open the same journal while the first is active"
        );
    }

    #[test]
    fn open_for_package_allows_reopening_after_the_first_writer_is_dropped() {
        let temp_root = tempdir().expect("temp root");
        prepare_journal_dir(temp_root.path(), "Contoso.App", "1.0.0");

        {
            let _first = JournalWriter::open_for_package(temp_root.path(), "Contoso.App", "1.0.0")
                .expect("first writer should open the journal");
        }

        JournalWriter::open_for_package(temp_root.path(), "Contoso.App", "1.0.0")
            .expect("a new writer should be able to open the journal once the lock is released");
    }
}
