use std::path::PathBuf;

use crate::fs::cleanup_path;

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
}

impl Drop for ExtractionCleanup {
    fn drop(&mut self) {
        while let Some(path) = self.created_files.pop() {
            let cleanup_result = cleanup_path(&path);

            #[cfg(debug_assertions)]
            if let Err(err) = &cleanup_result {
                eprintln!("cleanup failed for {}: {}", path.display(), err);
            }

            let _ = cleanup_result;
        }

        while let Some(path) = self.created_dirs.pop() {
            let cleanup_result = cleanup_path(&path);

            #[cfg(debug_assertions)]
            if let Err(err) = &cleanup_result {
                eprintln!("cleanup failed for {}: {}", path.display(), err);
            }

            let _ = cleanup_result;
        }
    }
}
