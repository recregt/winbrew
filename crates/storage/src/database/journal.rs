use anyhow::{Context, Result};
use core::str::FromStr;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use thiserror::Error;

use winbrew_models::{
    EngineKind, EngineMetadata, InstallerType, ModelError, Package, PackageStatus,
};

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
        #[serde(default)]
        package_id: String,
        #[serde(default)]
        version: String,
        #[serde(default)]
        engine: String,
        #[serde(default, skip_serializing_if = "String::is_empty")]
        install_dir: String,
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        dependencies: Vec<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        engine_metadata: Option<EngineMetadata>,
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
    Commit {
        #[serde(default, skip_serializing_if = "String::is_empty")]
        installed_at: String,
    },
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

            commit_seen |= matches!(entry, JournalEntry::Commit { .. });
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

#[derive(Debug, Clone)]
pub struct CommittedJournalPackage {
    pub journal_path: PathBuf,
    pub entries: Vec<JournalEntry>,
    pub package: Package,
}

#[derive(Debug, Error)]
pub enum JournalReplayError {
    #[error("failed to enumerate committed journals under {root}")]
    Enumerate {
        root: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error(transparent)]
    Read(#[from] JournalReadError),

    #[error("journal at {path} is missing metadata")]
    MissingMetadata { path: PathBuf },

    #[error("journal at {path} is missing commit marker")]
    MissingCommit { path: PathBuf },

    #[error("journal at {path} is missing required field {field}")]
    MissingField { path: PathBuf, field: &'static str },

    #[error("journal at {path} has invalid engine kind '{engine}'")]
    InvalidEngineKind {
        path: PathBuf,
        engine: String,
        #[source]
        source: ModelError,
    },
}

pub fn committed_journal_paths(root: &Path) -> Result<Vec<PathBuf>, JournalReplayError> {
    let pkgdb_dir = winbrew_core::pkgdb_dir_at(root);

    if !pkgdb_dir.exists() {
        return Ok(Vec::new());
    }

    let mut journal_paths = Vec::new();

    for entry in fs::read_dir(&pkgdb_dir).map_err(|source| JournalReplayError::Enumerate {
        root: pkgdb_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| JournalReplayError::Enumerate {
            root: pkgdb_dir.clone(),
            source,
        })?;

        let journal_path = entry.path().join("journal.jsonl");
        if journal_path.is_file() && JournalReader::read_committed(&journal_path).is_ok() {
            journal_paths.push(journal_path);
        }
    }

    journal_paths.sort();
    Ok(journal_paths)
}

pub fn read_committed_package_journal(
    path: &Path,
) -> Result<CommittedJournalPackage, JournalReplayError> {
    let entries = JournalReader::read_committed(path)?;

    let (package_id, version, engine, install_dir, dependencies, engine_metadata) = entries
        .iter()
        .find_map(|entry| match entry {
            JournalEntry::Metadata {
                package_id,
                version,
                engine,
                install_dir,
                dependencies,
                engine_metadata,
            } => Some((
                package_id.as_str(),
                version.as_str(),
                engine.as_str(),
                install_dir.as_str(),
                dependencies.clone(),
                engine_metadata.clone(),
            )),
            _ => None,
        })
        .ok_or_else(|| JournalReplayError::MissingMetadata {
            path: path.to_path_buf(),
        })?;

    if package_id.is_empty() {
        return Err(JournalReplayError::MissingField {
            path: path.to_path_buf(),
            field: "package_id",
        });
    }

    if version.is_empty() {
        return Err(JournalReplayError::MissingField {
            path: path.to_path_buf(),
            field: "version",
        });
    }

    if engine.is_empty() {
        return Err(JournalReplayError::MissingField {
            path: path.to_path_buf(),
            field: "engine",
        });
    }

    if install_dir.is_empty() {
        return Err(JournalReplayError::MissingField {
            path: path.to_path_buf(),
            field: "install_dir",
        });
    }

    let engine_kind =
        EngineKind::from_str(engine).map_err(|source| JournalReplayError::InvalidEngineKind {
            path: path.to_path_buf(),
            engine: engine.to_string(),
            source,
        })?;

    let installed_at = entries
        .iter()
        .rev()
        .find_map(|entry| match entry {
            JournalEntry::Commit { installed_at } => Some(installed_at.as_str()),
            _ => None,
        })
        .ok_or_else(|| JournalReplayError::MissingCommit {
            path: path.to_path_buf(),
        })?;

    if installed_at.is_empty() {
        return Err(JournalReplayError::MissingField {
            path: path.to_path_buf(),
            field: "installed_at",
        });
    }

    let package = Package {
        name: package_id.to_string(),
        version: version.to_string(),
        kind: InstallerType::from(engine_kind),
        engine_kind,
        engine_metadata,
        install_dir: install_dir.to_string(),
        dependencies,
        status: PackageStatus::Ok,
        installed_at: installed_at.to_string(),
    };

    Ok(CommittedJournalPackage {
        journal_path: path.to_path_buf(),
        entries,
        package,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        FileHash, HashAlgo, JournalEntry, JournalReadError, JournalReader, JournalWriter,
        committed_journal_paths, read_committed_package_journal,
    };
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};
    use winbrew_models::InstallerType;

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

    fn metadata_entry() -> JournalEntry {
        JournalEntry::Metadata {
            package_id: "winget/Contoso.App".to_string(),
            version: "1.0.0".to_string(),
            engine: "msi".to_string(),
            install_dir: r"C:\winbrew\apps\Contoso.App".to_string(),
            dependencies: vec!["winget/Contoso.Shared".to_string()],
            engine_metadata: None,
        }
    }

    fn commit_entry() -> JournalEntry {
        JournalEntry::Commit {
            installed_at: "2024-01-01T00:00:00Z".to_string(),
        }
    }
    #[test]
    fn journal_entries_are_written_as_jsonl() {
        let root = temp_root();
        let mut writer = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        writer.append(&metadata_entry()).expect("write metadata");
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
        writer.append(&commit_entry()).expect("write commit");
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
            JournalEntry::Commit { .. }
        ));
    }

    #[test]
    fn journal_reader_requires_commit_marker() {
        let root = temp_root();
        let mut writer = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        writer.append(&metadata_entry()).expect("write metadata");
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

        writer.append(&commit_entry()).expect("write commit");
        writer
            .append(&metadata_entry())
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

        writer.append(&commit_entry()).expect("write commit");
        writer.flush().expect("flush journal");

        let entries = JournalReader::read_committed(writer.path())
            .expect("commit-only journal should be accepted");

        assert_eq!(entries, vec![commit_entry()]);
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

        writer.append(&commit_entry()).expect("write commit");
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

        writer.append(&metadata_entry()).expect("write metadata");
        writer.flush().expect("flush journal");
        let journal_path = writer.path().to_path_buf();
        drop(writer);

        let mut resumed = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("resume incomplete journal");
        resumed.append(&commit_entry()).expect("write commit");
        resumed.flush().expect("flush resumed journal");

        let parsed = JournalReader::read_committed(&journal_path).expect("read committed journal");

        assert_eq!(parsed, vec![metadata_entry(), commit_entry(),]);
    }

    #[test]
    fn journal_round_trip_preserves_entries() {
        let root = temp_root();
        let mut writer = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        let original = vec![
            metadata_entry(),
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
            commit_entry(),
        ];

        for entry in &original {
            writer.append(entry).expect("write entry");
        }
        writer.flush().expect("flush journal");

        let parsed = JournalReader::read_committed(writer.path()).expect("read journal");

        assert_eq!(parsed, original);
    }

    #[test]
    fn committed_journal_paths_returns_only_committed_journals() {
        let root = temp_root();

        let mut committed =
            JournalWriter::open_for_package(&root, "winget/Contoso.Committed", "1.0.0")
                .expect("open committed journal");
        committed
            .append(&JournalEntry::Metadata {
                package_id: "winget/Contoso.Committed".to_string(),
                version: "1.0.0".to_string(),
                engine: "msi".to_string(),
                install_dir: r"C:\winbrew\apps\Contoso.Committed".to_string(),
                dependencies: Vec::new(),
                engine_metadata: None,
            })
            .expect("write committed metadata");
        committed.append(&commit_entry()).expect("write commit");
        committed.flush().expect("flush committed journal");

        let mut incomplete =
            JournalWriter::open_for_package(&root, "winget/Contoso.Incomplete", "1.0.0")
                .expect("open incomplete journal");
        incomplete
            .append(&JournalEntry::Metadata {
                package_id: "winget/Contoso.Incomplete".to_string(),
                version: "1.0.0".to_string(),
                engine: "msi".to_string(),
                install_dir: r"C:\winbrew\apps\Contoso.Incomplete".to_string(),
                dependencies: Vec::new(),
                engine_metadata: None,
            })
            .expect("write incomplete metadata");
        incomplete.flush().expect("flush incomplete journal");

        let journal_paths = committed_journal_paths(&root).expect("enumerate committed journals");

        assert_eq!(journal_paths, vec![committed.path().to_path_buf()]);
    }

    #[test]
    fn read_committed_package_journal_parses_snapshot() {
        let root = temp_root();
        let mut writer = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        writer.append(&metadata_entry()).expect("write metadata");
        writer.append(&commit_entry()).expect("write commit");
        writer.flush().expect("flush journal");

        let replay = read_committed_package_journal(writer.path()).expect("parse replay journal");

        assert_eq!(replay.journal_path, writer.path());
        assert_eq!(replay.entries.len(), 2);
        assert_eq!(replay.package.name, "winget/Contoso.App");
        assert_eq!(replay.package.version, "1.0.0");
        assert_eq!(replay.package.kind, InstallerType::Msi);
        assert_eq!(replay.package.install_dir, r"C:\winbrew\apps\Contoso.App");
        assert_eq!(
            replay.package.dependencies,
            vec!["winget/Contoso.Shared".to_string()]
        );
        assert_eq!(replay.package.installed_at, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn hash_algo_reports_security_profile() {
        assert!(!HashAlgo::Md5.is_secure());
        assert!(!HashAlgo::Sha1.is_secure());
        assert!(HashAlgo::Sha256.is_secure());
        assert!(HashAlgo::Sha512.is_secure());
    }
}
