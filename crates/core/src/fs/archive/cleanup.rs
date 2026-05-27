use std::path::{Path, PathBuf};

use crate::fs::cleanup_path;
use tracing::warn;

pub(crate) struct ExtractionCleanup {
    created_files: Vec<PathBuf>,
    created_dirs: Vec<PathBuf>,
}

impl ExtractionCleanup {
    pub(crate) fn new() -> Self {
        Self {
            created_files: Vec::new(),
            created_dirs: Vec::new(),
        }
    }

    pub(crate) fn record_file(&mut self, path: PathBuf) {
        self.created_files.push(path);
    }

    pub(crate) fn record_directory(&mut self, path: PathBuf) {
        self.created_dirs.push(path);
    }

    pub(crate) fn commit(mut self) {
        self.created_files.clear();
        self.created_dirs.clear();
    }

    fn cleanup_recorded_path(path: &Path) {
        if let Err(err) = cleanup_path(path) {
            warn!(path = %path.display(), error = %err, "cleanup failed");
        }
    }
}

impl Drop for ExtractionCleanup {
    fn drop(&mut self) {
        for path in self.created_files.iter().rev() {
            Self::cleanup_recorded_path(path);
        }

        for path in self.created_dirs.iter().rev() {
            Self::cleanup_recorded_path(path);
        }
    }
}
