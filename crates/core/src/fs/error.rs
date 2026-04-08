use std::error::Error as StdError;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

pub type BoxError = Box<dyn StdError + Send + Sync + 'static>;

#[derive(Debug, Error)]
pub enum FsError {
    #[error("failed to inspect {path}")]
    Inspect {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to remove {path}")]
    Remove {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to remove {path} and defer deletion to {deferred_path}")]
    RemoveAndDefer {
        path: PathBuf,
        deferred_path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to create directory {path}")]
    CreateDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("{action} {path}")]
    Io {
        action: &'static str,
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to create temp file at {path}")]
    CreateTempFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to write temp file at {path}")]
    WriteTempFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to sync temp file at {path}")]
    SyncTempFile {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to finalize file: {temp_path} -> {final_path}")]
    FinalizeFile {
        temp_path: PathBuf,
        final_path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to open zip archive {zip_path}")]
    OpenZipArchive {
        zip_path: PathBuf,
        #[source]
        source: BoxError,
    },

    #[error("failed to read zip entry for {path}")]
    ReadZipEntry {
        path: PathBuf,
        #[source]
        source: BoxError,
    },

    #[error("failed to read {path}")]
    ReadEntry {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to write {path}")]
    WriteEntry {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("zip entry contains an invalid path")]
    InvalidZipEntryPath,

    #[error("refusing to extract symlink entry {path}")]
    SymlinkEntry { path: PathBuf },

    #[error("refusing to create directory through reparse point {path}")]
    ReparsePoint { path: PathBuf },

    #[error("failed to create directory {path}: path exists and is not a directory")]
    PathNotDirectory { path: PathBuf },

    #[error("refusing to overwrite hardlinked file {path}")]
    HardlinkedTarget { path: PathBuf },

    #[error("failed to copy staged installation across volumes: {source_dir} -> {target_dir}")]
    CopyAcrossVolumes {
        source_dir: PathBuf,
        target_dir: PathBuf,
        #[source]
        source: BoxError,
    },

    #[error("failed to move staged installation into place: {source_dir} -> {target_dir}")]
    MoveIntoPlace {
        source_dir: PathBuf,
        target_dir: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to move existing installation aside: {target_dir} -> {backup_dir}")]
    MoveAside {
        target_dir: PathBuf,
        backup_dir: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error(
        "{action}: {source_dir} -> {target_dir} (original error: {source_error}; rollback also failed: {rollback_error})"
    )]
    RollbackFailed {
        action: &'static str,
        source_dir: PathBuf,
        target_dir: PathBuf,
        source_error: String,
        rollback_error: String,
        #[source]
        source: BoxError,
    },

    #[error("refusing to copy symlink {source_path}")]
    CopySymlink { source_path: PathBuf },

    #[error("unsupported entry type {source_path}")]
    UnsupportedEntry { source_path: PathBuf },
}

pub type Result<T> = std::result::Result<T, FsError>;

impl FsError {
    pub(crate) fn inspect(path: &Path, source: io::Error) -> Self {
        Self::Inspect {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn remove(path: &Path, source: io::Error) -> Self {
        Self::Remove {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn remove_and_defer(path: &Path, deferred_path: &Path, source: io::Error) -> Self {
        Self::RemoveAndDefer {
            path: path.to_path_buf(),
            deferred_path: deferred_path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn create_directory(path: &Path, source: io::Error) -> Self {
        Self::CreateDirectory {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn io(action: &'static str, path: &Path, source: io::Error) -> Self {
        Self::Io {
            action,
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn create_temp_file(path: &Path, source: io::Error) -> Self {
        Self::CreateTempFile {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn write_temp_file(path: &Path, source: io::Error) -> Self {
        Self::WriteTempFile {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn sync_temp_file(path: &Path, source: io::Error) -> Self {
        Self::SyncTempFile {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn finalize_file(temp_path: &Path, final_path: &Path, source: io::Error) -> Self {
        Self::FinalizeFile {
            temp_path: temp_path.to_path_buf(),
            final_path: final_path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn open_zip_archive(
        zip_path: &Path,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self::OpenZipArchive {
            zip_path: zip_path.to_path_buf(),
            source: Box::new(source),
        }
    }

    pub(crate) fn read_zip_entry(
        path: &Path,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self::ReadZipEntry {
            path: path.to_path_buf(),
            source: Box::new(source),
        }
    }

    pub(crate) fn read_entry(path: &Path, source: io::Error) -> Self {
        Self::ReadEntry {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn write_entry(path: &Path, source: io::Error) -> Self {
        Self::WriteEntry {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn invalid_zip_entry_path() -> Self {
        Self::InvalidZipEntryPath
    }

    pub(crate) fn symlink_entry(path: &Path) -> Self {
        Self::SymlinkEntry {
            path: path.to_path_buf(),
        }
    }

    pub(crate) fn reparse_point(path: &Path) -> Self {
        Self::ReparsePoint {
            path: path.to_path_buf(),
        }
    }

    pub(crate) fn path_not_directory(path: &Path) -> Self {
        Self::PathNotDirectory {
            path: path.to_path_buf(),
        }
    }

    pub(crate) fn hardlinked_target(path: &Path) -> Self {
        Self::HardlinkedTarget {
            path: path.to_path_buf(),
        }
    }

    pub(crate) fn copy_across_volumes(
        source_dir: &Path,
        target_dir: &Path,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self::CopyAcrossVolumes {
            source_dir: source_dir.to_path_buf(),
            target_dir: target_dir.to_path_buf(),
            source: Box::new(source),
        }
    }

    pub(crate) fn move_into_place(source_dir: &Path, target_dir: &Path, source: io::Error) -> Self {
        Self::MoveIntoPlace {
            source_dir: source_dir.to_path_buf(),
            target_dir: target_dir.to_path_buf(),
            source,
        }
    }

    pub(crate) fn move_aside(target_dir: &Path, backup_dir: &Path, source: io::Error) -> Self {
        Self::MoveAside {
            target_dir: target_dir.to_path_buf(),
            backup_dir: backup_dir.to_path_buf(),
            source,
        }
    }

    pub(crate) fn rollback_failed(
        action: &'static str,
        source_dir: &Path,
        target_dir: &Path,
        source_error: impl Into<String>,
        rollback_error: impl Into<String>,
        source: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self::RollbackFailed {
            action,
            source_dir: source_dir.to_path_buf(),
            target_dir: target_dir.to_path_buf(),
            source_error: source_error.into(),
            rollback_error: rollback_error.into(),
            source: Box::new(source),
        }
    }

    pub(crate) fn copy_symlink(source_path: &Path) -> Self {
        Self::CopySymlink {
            source_path: source_path.to_path_buf(),
        }
    }

    pub(crate) fn unsupported_entry(source_path: &Path) -> Self {
        Self::UnsupportedEntry {
            source_path: source_path.to_path_buf(),
        }
    }
}
