use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HashAlgo {
    Md5,
    Sha1,
    Sha256,
    Sha512,
}

impl HashAlgo {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Md5 => "md5",
            Self::Sha1 => "sha1",
            Self::Sha256 => "sha256",
            Self::Sha512 => "sha512",
        }
    }

    pub fn expected_hex_len(self) -> usize {
        match self {
            Self::Md5 => 32,
            Self::Sha1 => 40,
            Self::Sha256 => 64,
            Self::Sha512 => 128,
        }
    }

    pub fn is_secure(self) -> bool {
        matches!(self, Self::Sha256 | Self::Sha512)
    }
}

impl fmt::Display for HashAlgo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<winbrew_models::HashAlgorithm> for HashAlgo {
    fn from(value: winbrew_models::HashAlgorithm) -> Self {
        match value {
            winbrew_models::HashAlgorithm::Md5 => Self::Md5,
            winbrew_models::HashAlgorithm::Sha1 => Self::Sha1,
            winbrew_models::HashAlgorithm::Sha256 => Self::Sha256,
            winbrew_models::HashAlgorithm::Sha512 => Self::Sha512,
        }
    }
}

impl From<HashAlgo> for winbrew_models::HashAlgorithm {
    fn from(value: HashAlgo) -> Self {
        match value {
            HashAlgo::Md5 => Self::Md5,
            HashAlgo::Sha1 => Self::Sha1,
            HashAlgo::Sha256 => Self::Sha256,
            HashAlgo::Sha512 => Self::Sha512,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileHash {
    pub algo: HashAlgo,
    pub hex: String,
}

impl FileHash {
    pub fn new(algo: HashAlgo, hex: impl Into<String>) -> Self {
        let hex = hex.into();
        debug_assert!(
            hex.chars().all(|c| c.is_ascii_hexdigit()),
            "FileHash hex must be valid hexadecimal"
        );
        debug_assert_eq!(
            hex.len(),
            algo.expected_hex_len(),
            "FileHash hex length mismatch for {algo}"
        );

        Self { algo, hex }
    }

    pub fn as_prefixed_string(&self) -> String {
        format!("{}:{}", self.algo, self.hex)
    }
}

impl fmt::Display for FileHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.as_prefixed_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "snake_case")]
pub enum JournalEntry {
    Metadata {
        package_id: String,
        version: String,
        engine: String,
    },
    FsCreate {
        path: String,
        hash: Option<FileHash>,
    },
    FsDelete {
        path: String,
        hash: Option<FileHash>,
    },
    RegSet {
        hive: String,
        key: String,
        value: String,
        previous_value: Option<String>,
    },
    Shortcut {
        path: String,
        target: Option<String>,
    },
    Component {
        id: String,
        path: Option<String>,
    },
    Commit,
}

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

#[derive(Debug, Error)]
pub enum JournalReadError {
    #[error("failed to read journal at {path}")]
    Read {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("journal at {path} is incomplete")]
    Incomplete { path: PathBuf },

    #[error("journal at {path} is malformed on line {line}")]
    MalformedLine {
        path: PathBuf,
        line: usize,
        #[source]
        source: serde_json::Error,
    },

    #[error("journal at {path} has trailing entries after Commit on line {line}")]
    TrailingEntries { path: PathBuf, line: usize },
}

pub struct JournalReader;

impl JournalReader {
    pub fn read_committed(path: &Path) -> std::result::Result<Vec<JournalEntry>, JournalReadError> {
        let contents = fs::read_to_string(path).map_err(|source| JournalReadError::Read {
            path: path.to_path_buf(),
            source,
        })?;

        let lines = contents
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();

        if lines.is_empty() {
            return Err(JournalReadError::Incomplete {
                path: path.to_path_buf(),
            });
        }

        let mut entries = Vec::with_capacity(lines.len());
        let mut commit_seen = false;

        for (index, line) in lines.iter().enumerate() {
            if commit_seen {
                return Err(JournalReadError::TrailingEntries {
                    path: path.to_path_buf(),
                    line: index + 1,
                });
            }

            let entry = serde_json::from_str::<JournalEntry>(line).map_err(|source| {
                JournalReadError::MalformedLine {
                    path: path.to_path_buf(),
                    line: index + 1,
                    source,
                }
            })?;

            commit_seen |= matches!(entry, JournalEntry::Commit);
            entries.push(entry);
        }

        if !commit_seen {
            return Err(JournalReadError::Incomplete {
                path: path.to_path_buf(),
            });
        }

        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::{FileHash, HashAlgo, JournalEntry, JournalReadError, JournalReader, JournalWriter};
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root() -> PathBuf {
        let unique_id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after unix epoch")
            .as_nanos();

        std::env::temp_dir().join(format!(
            "winbrew-storage-journal-{}-{unique_id}",
            process::id()
        ))
    }

    #[test]
    fn journal_entries_are_written_as_jsonl() {
        let root = temp_root();
        let mut writer = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        writer
            .append(&JournalEntry::Metadata {
                package_id: "winget/Contoso.App".to_string(),
                version: "1.0.0".to_string(),
                engine: "msi".to_string(),
            })
            .expect("write metadata");
        writer
            .append(&JournalEntry::FsCreate {
                path: r"C:\winbrew\apps\Contoso.App\app.exe".to_string(),
                hash: Some(FileHash::new(
                    HashAlgo::Sha256,
                    "deadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeefdeadbeef",
                )),
            })
            .expect("write fs create");
        writer
            .append(&JournalEntry::RegSet {
                hive: "HKCU".to_string(),
                key: r"Software\Classes\*\shell\Open with Code".to_string(),
                value: "command".to_string(),
                previous_value: None,
            })
            .expect("write reg set");
        writer.append(&JournalEntry::Commit).expect("write commit");
        writer.flush().expect("flush journal");

        let contents = fs::read_to_string(writer.path()).expect("read journal");
        let lines = contents.lines().collect::<Vec<_>>();

        assert_eq!(lines.len(), 4);
        assert!(matches!(
            serde_json::from_str::<JournalEntry>(lines[0]).expect("parse metadata"),
            JournalEntry::Metadata { .. }
        ));
        assert!(matches!(
            serde_json::from_str::<JournalEntry>(lines[1]).expect("parse fs create"),
            JournalEntry::FsCreate { .. }
        ));
        assert!(matches!(
            serde_json::from_str::<JournalEntry>(lines[2]).expect("parse reg set"),
            JournalEntry::RegSet { .. }
        ));
        assert!(matches!(
            serde_json::from_str::<JournalEntry>(lines[3]).expect("parse commit"),
            JournalEntry::Commit
        ));
    }

    #[test]
    fn journal_reader_requires_commit_marker() {
        let root = temp_root();
        let mut writer = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        writer
            .append(&JournalEntry::Metadata {
                package_id: "winget/Contoso.App".to_string(),
                version: "1.0.0".to_string(),
                engine: "msi".to_string(),
            })
            .expect("write metadata");
        writer.flush().expect("flush journal");

        let err =
            JournalReader::read_committed(writer.path()).expect_err("journal should be incomplete");

        assert!(matches!(err, JournalReadError::Incomplete { .. }));
    }

    #[test]
    fn journal_reader_rejects_trailing_entries_after_commit() {
        let root = temp_root();
        let mut writer = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        writer.append(&JournalEntry::Commit).expect("write commit");
        writer
            .append(&JournalEntry::Metadata {
                package_id: "winget/Contoso.App".to_string(),
                version: "1.0.0".to_string(),
                engine: "msi".to_string(),
            })
            .expect("write trailing metadata");
        writer.flush().expect("flush journal");

        let err =
            JournalReader::read_committed(writer.path()).expect_err("journal should be rejected");

        assert!(matches!(err, JournalReadError::TrailingEntries { .. }));
    }

    #[test]
    fn journal_reader_rejects_empty_file() {
        let root = temp_root();
        let writer = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        fs::write(writer.path(), b"").expect("truncate journal to empty");

        let err = JournalReader::read_committed(writer.path())
            .expect_err("empty file should be incomplete");

        assert!(matches!(err, JournalReadError::Incomplete { .. }));
    }

    #[test]
    fn journal_reader_rejects_whitespace_only_file() {
        let root = temp_root();
        let writer = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        fs::write(writer.path(), b"\n  \n\t\n").expect("write whitespace-only journal");

        let err = JournalReader::read_committed(writer.path())
            .expect_err("whitespace-only file should be incomplete");

        assert!(matches!(err, JournalReadError::Incomplete { .. }));
    }

    // Metadata is not required here; doctor or replay code can validate it separately.
    #[test]
    fn journal_reader_accepts_commit_only() {
        let root = temp_root();
        let mut writer = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        writer.append(&JournalEntry::Commit).expect("write commit");
        writer.flush().expect("flush journal");

        let entries = JournalReader::read_committed(writer.path())
            .expect("commit-only journal should be accepted");

        assert_eq!(entries, vec![JournalEntry::Commit]);
    }

    #[test]
    fn journal_reader_rejects_malformed_line() {
        let root = temp_root();
        let writer = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        fs::write(
            writer.path(),
            b"{\"action\":\"metadata\",\"package_id\":\"winget/Contoso.App\",\"version\":\"1.0.0\",\"engine\":\"msi\"}\n{not-json}\n",
        )
        .expect("write malformed journal");

        let err = JournalReader::read_committed(writer.path())
            .expect_err("malformed line should be rejected");

        assert!(matches!(
            err,
            JournalReadError::MalformedLine { line: 2, .. }
        ));
    }

    #[test]
    fn open_for_package_rejects_already_committed_journal() {
        let root = temp_root();
        let mut writer = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        writer.append(&JournalEntry::Commit).expect("write commit");
        writer.flush().expect("flush journal");
        let journal_path = writer.path().to_path_buf();
        drop(writer);

        let err = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect_err("committed journal should be rejected");

        assert!(err.to_string().contains("already committed"));
        assert!(journal_path.exists());
    }

    #[test]
    fn open_for_package_allows_resuming_incomplete_journal() {
        let root = temp_root();
        let mut writer = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        writer
            .append(&JournalEntry::Metadata {
                package_id: "winget/Contoso.App".to_string(),
                version: "1.0.0".to_string(),
                engine: "msi".to_string(),
            })
            .expect("write metadata");
        writer.flush().expect("flush journal");
        let journal_path = writer.path().to_path_buf();
        drop(writer);

        let mut resumed = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("resume incomplete journal");
        resumed.append(&JournalEntry::Commit).expect("write commit");
        resumed.flush().expect("flush resumed journal");

        let parsed = JournalReader::read_committed(&journal_path).expect("read committed journal");

        assert_eq!(
            parsed,
            vec![
                JournalEntry::Metadata {
                    package_id: "winget/Contoso.App".to_string(),
                    version: "1.0.0".to_string(),
                    engine: "msi".to_string(),
                },
                JournalEntry::Commit,
            ]
        );
    }

    #[test]
    fn journal_round_trip_preserves_entries() {
        let root = temp_root();
        let mut writer = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        let original = vec![
            JournalEntry::Metadata {
                package_id: "winget/Contoso.App".to_string(),
                version: "1.0.0".to_string(),
                engine: "msi".to_string(),
            },
            JournalEntry::FsCreate {
                path: r"C:\winbrew\apps\Contoso.App\app.exe".to_string(),
                hash: Some(FileHash::new(
                    HashAlgo::Sha512,
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                )),
            },
            JournalEntry::FsDelete {
                path: r"C:\winbrew\apps\Contoso.App\old.exe".to_string(),
                hash: None,
            },
            JournalEntry::Commit,
        ];

        for entry in &original {
            writer.append(entry).expect("write entry");
        }
        writer.flush().expect("flush journal");

        let parsed = JournalReader::read_committed(writer.path()).expect("read journal");

        assert_eq!(parsed, original);
    }

    #[test]
    fn hash_algo_reports_security_profile() {
        assert!(!HashAlgo::Md5.is_secure());
        assert!(!HashAlgo::Sha1.is_secure());
        assert!(HashAlgo::Sha256.is_secure());
        assert!(HashAlgo::Sha512.is_secure());
    }
}
