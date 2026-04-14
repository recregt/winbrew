use serde::{Deserialize, Serialize};
use std::fmt;

use winbrew_models::install::engine::EngineMetadata;
use winbrew_models::shared::DeploymentKind;
use winbrew_models::shared::hash::HashAlgorithm;

mod reader;
mod replay;
mod writer;

pub use reader::{JournalReadError, JournalReader};
pub use replay::{CommittedJournalPackage, JournalReplayError};
pub use writer::JournalWriter;

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

impl From<HashAlgorithm> for HashAlgo {
    fn from(value: HashAlgorithm) -> Self {
        match value {
            HashAlgorithm::Md5 => Self::Md5,
            HashAlgorithm::Sha1 => Self::Sha1,
            HashAlgorithm::Sha256 => Self::Sha256,
            HashAlgorithm::Sha512 => Self::Sha512,
        }
    }
}

impl From<HashAlgo> for HashAlgorithm {
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
        deployment_kind: DeploymentKind,
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

#[cfg(test)]
mod tests {
    use super::{FileHash, HashAlgo, JournalEntry, JournalReadError, JournalReader, JournalWriter};
    use std::fs;
    use std::path::Path;
    use std::path::PathBuf;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};
    use winbrew_core::{ResolvedPaths, package_journal_key, resolved_paths};
    use winbrew_models::install::installer::InstallerType;

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

    fn resolved_root_paths(root: &Path) -> ResolvedPaths {
        let packages = root.join("packages").to_string_lossy().into_owned();
        let data = root.join("data").to_string_lossy().into_owned();
        let logs = root.join("logs").to_string_lossy().into_owned();
        let cache = root.join("cache").to_string_lossy().into_owned();

        resolved_paths(root, &packages, &data, &logs, &cache)
    }

    fn metadata_entry() -> JournalEntry {
        JournalEntry::Metadata {
            package_id: "winget/Contoso.App".to_string(),
            version: "1.0.0".to_string(),
            engine: "msi".to_string(),
            deployment_kind: winbrew_models::shared::DeploymentKind::Installed,
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
    fn journal_entries_are_written_under_resolved_paths() {
        let root = temp_root();
        let paths = resolved_root_paths(&root);
        let package_key = package_journal_key("winget/Contoso.App", "1.0.0");

        let writer = JournalWriter::open_for_package_in(&paths, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        assert_eq!(writer.path(), paths.package_journal_file(&package_key));
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
    fn read_committed_package_journal_ignores_trailing_entries() {
        let root = temp_root();
        let mut writer = JournalWriter::open_for_package(&root, "winget/Contoso.App", "1.0.0")
            .expect("open journal");

        writer.append(&metadata_entry()).expect("write metadata");
        writer.append(&commit_entry()).expect("write commit");
        writer
            .append(&JournalEntry::FsCreate {
                path: r"C:\winbrew\apps\Contoso.App\payload.exe".to_string(),
                hash: None,
            })
            .expect("write trailing entry");
        writer.flush().expect("flush journal");

        let replay = JournalReader::read_committed_package(writer.path())
            .expect("parse replay journal with trailing entries");

        assert_eq!(replay.journal_path, writer.path());
        assert_eq!(replay.entries, vec![metadata_entry(), commit_entry()]);
        assert_eq!(replay.package.name, "winget/Contoso.App");
        assert_eq!(replay.package.version, "1.0.0");
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
            b"{\"action\":\"metadata\",\"package_id\":\"winget/Contoso.App\",\"version\":\"1.0.0\",\"engine\":\"msi\",\"deployment_kind\":\"installed\"}\n{not-json}\n",
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
        let paths = resolved_root_paths(&root);

        let mut committed =
            JournalWriter::open_for_package(&root, "winget/Contoso.Committed", "1.0.0")
                .expect("open committed journal");
        committed
            .append(&JournalEntry::Metadata {
                package_id: "winget/Contoso.Committed".to_string(),
                version: "1.0.0".to_string(),
                engine: "msi".to_string(),
                deployment_kind: winbrew_models::shared::DeploymentKind::Installed,
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
                deployment_kind: winbrew_models::shared::DeploymentKind::Installed,
                install_dir: r"C:\winbrew\apps\Contoso.Incomplete".to_string(),
                dependencies: Vec::new(),
                engine_metadata: None,
            })
            .expect("write incomplete metadata");
        incomplete.flush().expect("flush incomplete journal");

        let journal_paths =
            JournalReader::committed_paths_in(&paths).expect("enumerate committed journals");

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

        let replay =
            JournalReader::read_committed_package(writer.path()).expect("parse replay journal");

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
