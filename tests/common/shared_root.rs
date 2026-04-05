use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::sync::atomic::{AtomicUsize, Ordering};

static SHARED_ROOT: OnceLock<PathBuf> = OnceLock::new();
static SHARED_ROOT_SUFFIX: AtomicUsize = AtomicUsize::new(0);

pub fn shared_test_root() -> &'static Path {
    SHARED_ROOT
        .get_or_init(|| {
            let suffix = SHARED_ROOT_SUFFIX.fetch_add(1, Ordering::Relaxed);
            let root = std::env::temp_dir().join(format!(
                "winbrew-tests-{}-{}",
                std::process::id(),
                suffix
            ));

            if root.exists() {
                let _ = fs::remove_dir_all(&root);
            }

            fs::create_dir_all(&root).expect("failed to create shared test root");

            unsafe {
                std::env::set_var("WINBREW_PATHS_ROOT", &root);
            }

            root
        })
        .as_path()
}
