use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

use super::JournalEntry;

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
