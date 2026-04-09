use std::error::Error as StdError;
use std::io;
use std::path::{Path, PathBuf};

use thiserror::Error;

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

    /// Raised when immediate deletion fails and the path cannot be renamed for deferred cleanup.
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

    #[error("failed to create extracted file {path}")]
    CreateExtractedFile {
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
        source: Box<dyn StdError + Send + Sync + 'static>,
    },

    #[error("failed to read zip entry for {path}")]
    ReadZipEntry {
        path: PathBuf,
        #[source]
        source: Box<dyn StdError + Send + Sync + 'static>,
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

    #[error("failed to read directory {path}")]
    ReadDirectory {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to read directory entry in {path}")]
    ReadDirectoryEntry {
        path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("failed to copy file {source_path} -> {target_path}")]
    CopyFile {
        source_path: PathBuf,
        target_path: PathBuf,
        #[source]
        source: io::Error,
    },

    #[error("zip entry contains an invalid path")]
    InvalidZipEntryPath,

    #[error("refusing to extract symlink entry {path}")]
    SymlinkEntry { path: PathBuf },

    #[error(
        "suspicious compression ratio for {path}: {uncompressed_size} bytes from {compressed_size} compressed bytes exceeds {max_ratio}x"
    )]
    SuspiciousCompressionRatio {
        path: PathBuf,
        uncompressed_size: u64,
        compressed_size: u64,
        max_ratio: u64,
    },

    #[error(
        "zip extraction quota exceeded: current total {current_total_size} bytes + entry {entry_size} bytes exceeds limit {max_total_size}"
    )]
    QuotaExceeded {
        max_total_size: u64,
        current_total_size: u64,
        entry_size: u64,
    },

    #[error("zip entry path is too deep {path}: depth {depth} exceeds limit {max_depth}")]
    PathTooDeep {
        path: PathBuf,
        depth: usize,
        max_depth: usize,
    },

    #[error(
        "zip extraction entry count exceeded: current count {current_file_count} exceeds limit {max_file_count}"
    )]
    FileCountExceeded {
        max_file_count: usize,
        current_file_count: usize,
    },

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
        source: Box<dyn StdError + Send + Sync + 'static>,
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

    /// Raised when staged replacement fails and restoring the backup also fails.
    #[error(
        "{action}: {source_dir} -> {target_dir} (original error: {source}; rollback also failed: {rollback_error})"
    )]
    RollbackFailed {
        action: &'static str,
        source_dir: PathBuf,
        target_dir: PathBuf,
        #[source]
        source: Box<dyn StdError + Send + Sync + 'static>,
        rollback_error: Box<dyn StdError + Send + Sync + 'static>,
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

    pub(crate) fn create_extracted_file(path: &Path, source: io::Error) -> Self {
        Self::CreateExtractedFile {
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

    pub(crate) fn read_directory(path: &Path, source: io::Error) -> Self {
        Self::ReadDirectory {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn read_directory_entry(path: &Path, source: io::Error) -> Self {
        Self::ReadDirectoryEntry {
            path: path.to_path_buf(),
            source,
        }
    }

    pub(crate) fn copy_file(source_path: &Path, target_path: &Path, source: io::Error) -> Self {
        Self::CopyFile {
            source_path: source_path.to_path_buf(),
            target_path: target_path.to_path_buf(),
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

    pub(crate) fn suspicious_compression_ratio(
        path: &Path,
        uncompressed_size: u64,
        compressed_size: u64,
        max_ratio: u64,
    ) -> Self {
        Self::SuspiciousCompressionRatio {
            path: path.to_path_buf(),
            uncompressed_size,
            compressed_size,
            max_ratio,
        }
    }

    pub(crate) fn quota_exceeded(
        max_total_size: u64,
        current_total_size: u64,
        entry_size: u64,
    ) -> Self {
        Self::QuotaExceeded {
            max_total_size,
            current_total_size,
            entry_size,
        }
    }

    pub(crate) fn path_too_deep(path: &Path, depth: usize, max_depth: usize) -> Self {
        Self::PathTooDeep {
            path: path.to_path_buf(),
            depth,
            max_depth,
        }
    }

    pub(crate) fn file_count_exceeded(max_file_count: usize, current_file_count: usize) -> Self {
        Self::FileCountExceeded {
            max_file_count,
            current_file_count,
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
        source: impl StdError + Send + Sync + 'static,
        rollback_error: impl StdError + Send + Sync + 'static,
    ) -> Self {
        Self::RollbackFailed {
            action,
            source_dir: source_dir.to_path_buf(),
            target_dir: target_dir.to_path_buf(),
            source: Box::new(source),
            rollback_error: Box::new(rollback_error),
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
