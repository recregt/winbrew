use anyhow::Result;
use core::str::FromStr;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

use super::{JournalEntry, JournalReadError, JournalReader};
use winbrew_models::{EngineKind, InstallerType, ModelError, Package, PackageStatus};

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

impl JournalReader {
    pub fn committed_paths(root: &Path) -> Result<Vec<PathBuf>, JournalReplayError> {
        enumerate_committed_journals(root)
    }

    pub fn read_committed_package(
        path: &Path,
    ) -> Result<CommittedJournalPackage, JournalReplayError> {
        parse_committed_package_journal(path)
    }
}

fn enumerate_committed_journals(root: &Path) -> Result<Vec<PathBuf>, JournalReplayError> {
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

fn parse_committed_package_journal(
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
