//! Update command wrapper for refreshing the catalog bundle.
//!
//! The command owns the progress-bar wiring and the final success message
//! while the app layer performs the download, verification, and file swap.

use anyhow::Result;
use std::io::Write;

use crate::core::paths::ResolvedPaths;
use crate::{CommandContext, app::update};
use winbrew_ui::Ui;

pub fn run(ctx: &CommandContext) -> Result<()> {
    let mut ui = ctx.ui();
    ui.page_title("Update Package Catalog");

    run_with_refresher(&mut ui, &ctx.app().paths, &RealCatalogRefresher)
}

fn run_with_refresher<W, R>(ui: &mut Ui<W>, paths: &ResolvedPaths, refresher: &R) -> Result<()>
where
    W: Write,
    R: CatalogRefresher + ?Sized,
{
    let progress = ui.progress_bar();

    let result = refresher.refresh_catalog(
        paths,
        |total_bytes| {
            if let Some(total_bytes) = total_bytes {
                progress.set_length(total_bytes);
            }
            progress.set_message("Downloading catalog bundle");
        },
        |downloaded_bytes| {
            progress.inc(downloaded_bytes);
        },
    );

    progress.finish_and_clear();
    result?;

    ui.success("Package catalog updated.");
    Ok(())
}

trait CatalogRefresher {
    fn refresh_catalog<FStart, FProgress>(
        &self,
        paths: &ResolvedPaths,
        on_start: FStart,
        on_progress: FProgress,
    ) -> Result<()>
    where
        FStart: FnOnce(Option<u64>),
        FProgress: FnMut(u64);
}

struct RealCatalogRefresher;

impl CatalogRefresher for RealCatalogRefresher {
    fn refresh_catalog<FStart, FProgress>(
        &self,
        paths: &ResolvedPaths,
        on_start: FStart,
        on_progress: FProgress,
    ) -> Result<()>
    where
        FStart: FnOnce(Option<u64>),
        FProgress: FnMut(u64),
    {
        update::refresh_catalog(paths, on_start, on_progress)
    }
}

#[cfg(test)]
mod tests {
    use super::{CatalogRefresher, run_with_refresher};
    use crate::core::paths::{ResolvedPaths, resolved_paths};
    use anyhow::Result;
    use std::io::{self, Write};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex};
    use tempfile::tempdir;
    use winbrew_ui::{UiBuilder, UiSettings};

    struct SharedBuffer {
        bytes: Arc<Mutex<Vec<u8>>>,
    }

    impl SharedBuffer {
        fn new(bytes: Arc<Mutex<Vec<u8>>>) -> Self {
            Self { bytes }
        }
    }

    impl Write for SharedBuffer {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.bytes
                .lock()
                .expect("buffer lock should not be poisoned")
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    struct FakeCatalogRefresher {
        expected_root: PathBuf,
        called: Arc<AtomicBool>,
    }

    impl CatalogRefresher for FakeCatalogRefresher {
        fn refresh_catalog<FStart, FProgress>(
            &self,
            paths: &ResolvedPaths,
            on_start: FStart,
            on_progress: FProgress,
        ) -> Result<()>
        where
            FStart: FnOnce(Option<u64>),
            FProgress: FnMut(u64),
        {
            assert_eq!(paths.root, self.expected_root);

            on_start(Some(64));
            let mut on_progress = on_progress;
            on_progress(64);

            self.called.store(true, Ordering::Relaxed);
            Ok(())
        }
    }

    #[test]
    fn update_run_reports_success_after_refresh() {
        let temp_dir = tempdir().expect("temp dir");
        let paths = resolved_paths(
            temp_dir.path(),
            "${root}/packages",
            "${root}/data",
            "${root}/data/logs",
            "${root}/data/cache",
        );

        let output = Arc::new(Mutex::new(Vec::new()));
        let error_output = Arc::new(Mutex::new(Vec::new()));
        let writer = SharedBuffer::new(output.clone());
        let error_writer = SharedBuffer::new(error_output.clone());
        let mut ui = UiBuilder::with_writer(writer, UiSettings::default())
            .with_error_writer(Box::new(error_writer))
            .color_enabled(false)
            .build();
        let called = Arc::new(AtomicBool::new(false));
        let refresher = FakeCatalogRefresher {
            expected_root: temp_dir.path().to_path_buf(),
            called: called.clone(),
        };

        run_with_refresher(&mut ui, &paths, &refresher).expect("update should succeed");

        assert!(called.load(Ordering::Relaxed));

        let error_output = error_output
            .lock()
            .expect("buffer lock should not be poisoned");
        let text = String::from_utf8(error_output.clone()).expect("output should be utf-8");

        assert!(text.contains("Package catalog updated."));
    }
}
