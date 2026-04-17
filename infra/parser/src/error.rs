use std::io;
use std::path::PathBuf;

use rusqlite::Error as SqliteError;
use thiserror::Error;

use winbrew_models::shared::error::ModelError;

#[derive(Debug, Error)]
pub enum ParserError {
    #[error("failed to decode fetched package payload")]
    Decode(#[from] serde_json::Error),

    #[error("invalid scoop stream contract: {0}")]
    Contract(String),

    #[error("failed to decode fetched package payload on line {line}")]
    LineDecode {
        line: usize,
        #[source]
        source: serde_json::Error,
    },

    #[error("failed to read or write parser artifact")]
    Io(#[from] io::Error),

    #[error("failed to access catalog database at {path}")]
    CatalogDb {
        path: PathBuf,
        #[source]
        source: SqliteError,
    },

    #[error(transparent)]
    Model(#[from] ModelError),
}
