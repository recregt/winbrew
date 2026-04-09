use std::collections::HashMap;
use std::fs;
use std::io::ErrorKind;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use crate::fs::{FsError, Result};

use super::super::platform::PlatformAdapter;
use super::{CachedPath, ExtractionCleanup, ExtractionLimits, PathInfo};

pub(crate) struct ExtractionContext<P: PlatformAdapter> {
    cached_paths: HashMap<PathBuf, CachedPath>,
    cleanup: ExtractionCleanup,
    limits: ExtractionLimits,
    current_total_size: u64,
    current_file_count: usize,
    platform: PhantomData<P>,
}

impl<P: PlatformAdapter> ExtractionContext<P> {
    pub(crate) fn new(limits: ExtractionLimits) -> Self {
        Self {
            cached_paths: HashMap::new(),
            cleanup: ExtractionCleanup::new(),
            limits,
            current_total_size: 0,
            current_file_count: 0,
            platform: PhantomData,
        }
    }

    pub(crate) fn commit(self) {
        self.cleanup.commit();
    }

    pub(crate) fn validate_target(&mut self, path: &Path, destination_dir: &Path) -> Result<()> {
        let mut current = Some(path);
        let mut is_final_component = true;

        while let Some(candidate) = current {
            match self.inspect_cached(candidate)? {
                CachedPath::Present(info) => {
                    if info.is_reparse_point {
                        return Err(FsError::reparse_point(candidate));
                    }

                    if is_final_component && !info.is_directory && info.hard_link_count > 1 {
                        return Err(FsError::hardlinked_target(candidate));
                    }
                }
                CachedPath::Missing => {}
            }

            if candidate == destination_dir {
                break;
            }

            is_final_component = false;
            current = candidate.parent();
        }

        Ok(())
    }

    pub(crate) fn ensure_directory_tree(&mut self, path: &Path) -> Result<()> {
        let mut missing_directories = Vec::new();
        let mut current = Some(path);

        while let Some(candidate) = current {
            match self.inspect_cached(candidate)? {
                CachedPath::Present(info) => {
                    if !info.is_directory {
                        return Err(FsError::path_not_directory(candidate));
                    }

                    break;
                }
                CachedPath::Missing => {
                    missing_directories.push(candidate.to_path_buf());
                    current = candidate.parent();
                }
            }
        }

        // missing_directories is collected deepest-first, so the first entry is the
        // deepest full path and create_dir_all on it materializes the whole chain.
        if let Some(deepest_missing) = missing_directories.first() {
            fs::create_dir_all(deepest_missing)
                .map_err(|err| FsError::create_directory(deepest_missing, err))?;

            // Record parents before children so drop can clean up deepest paths first.
            for directory in missing_directories.iter().rev() {
                self.record_directory(directory);
            }
        }

        Ok(())
    }

    pub(crate) fn check_limits(
        &mut self,
        path: &Path,
        entry_size: u64,
        compressed_size: u64,
    ) -> Result<()> {
        let path_depth = path.components().count();

        if path_depth > self.limits.max_path_depth {
            return Err(FsError::path_too_deep(
                path,
                path_depth,
                self.limits.max_path_depth,
            ));
        }

        // Empty entries do not have a meaningful compression ratio, but they still
        // count toward quota and entry-count limits.
        if entry_size > 0
            && (compressed_size == 0
                || entry_size > compressed_size.saturating_mul(self.limits.max_compression_ratio))
        {
            return Err(FsError::suspicious_compression_ratio(
                path,
                entry_size,
                compressed_size,
                self.limits.max_compression_ratio,
            ));
        }

        let new_total_size = self
            .current_total_size
            .checked_add(entry_size)
            .ok_or_else(|| {
                FsError::quota_exceeded(
                    self.limits.max_total_size,
                    self.current_total_size,
                    entry_size,
                )
            })?;
        if new_total_size > self.limits.max_total_size {
            return Err(FsError::quota_exceeded(
                self.limits.max_total_size,
                self.current_total_size,
                entry_size,
            ));
        }

        let new_file_count = self.current_file_count.checked_add(1).ok_or_else(|| {
            FsError::file_count_exceeded(self.limits.max_file_count, self.current_file_count)
        })?;
        if new_file_count > self.limits.max_file_count {
            return Err(FsError::file_count_exceeded(
                self.limits.max_file_count,
                self.current_file_count,
            ));
        }

        self.current_total_size = new_total_size;
        self.current_file_count = new_file_count;
        Ok(())
    }

    pub(crate) fn record_file(&mut self, path: &Path) {
        self.cached_paths.insert(
            path.to_path_buf(),
            CachedPath::Present(PathInfo {
                is_directory: false,
                is_reparse_point: false,
                hard_link_count: 1,
            }),
        );
        self.cleanup.record_file(path.to_path_buf());
    }

    fn record_directory(&mut self, path: &Path) {
        self.cached_paths.insert(
            path.to_path_buf(),
            CachedPath::Present(PathInfo {
                is_directory: true,
                is_reparse_point: false,
                hard_link_count: 1,
            }),
        );
        self.cleanup.record_directory(path.to_path_buf());
    }

    fn inspect_cached(&mut self, path: &Path) -> Result<CachedPath> {
        if let Some(cached) = self.cached_paths.get(path) {
            return Ok(*cached);
        }

        let state = match P::inspect_path(path) {
            Ok(info) => CachedPath::Present(info),
            Err(err) if err.kind() == ErrorKind::NotFound => CachedPath::Missing,
            Err(err) => return Err(FsError::inspect(path, err)),
        };

        self.cached_paths.insert(path.to_path_buf(), state);
        Ok(state)
    }
}
