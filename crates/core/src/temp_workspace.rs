//! Temporary workspace helpers for download and install flows.
//!
//! The temp-workspace contract keeps installer staging under a repo-independent
//! directory derived from the OS temp root. That lets the download and install
//! pipelines create disposable workspaces without polluting the managed root.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

static TEMP_ROOT_SUFFIX: AtomicUsize = AtomicUsize::new(0);

/// Build a unique temp root for a package name and version.
pub fn build_temp_root(name: &str, version: &str) -> PathBuf {
    temp_root_base().join(temp_root_name(name, version))
}

/// Return the stable prefix used for temp root detection.
pub fn temp_root_prefix(name: &str, version: &str) -> String {
    format!(
        "winbrew-install-{}-{}-",
        sanitize_component(name),
        sanitize_component(version)
    )
}

/// Return the base temp directory used by WinBrew staging workspaces.
pub fn temp_root_base() -> PathBuf {
    std::env::temp_dir().join("winbrew")
}

/// Return `true` when a path belongs to the expected package temp root.
pub fn is_temp_root_for(name: &str, version: &str, path: &Path) -> bool {
    path.file_name()
        .and_then(|value| value.to_str())
        .map(|file_name| file_name.starts_with(&temp_root_prefix(name, version)))
        .unwrap_or(false)
}

fn temp_root_name(name: &str, version: &str) -> String {
    let suffix = TEMP_ROOT_SUFFIX.fetch_add(1, Ordering::Relaxed);
    let mut segment = String::with_capacity(temp_root_prefix(name, version).len() + 20);
    segment.push_str(&temp_root_prefix(name, version));
    segment.push_str(&std::process::id().to_string());
    segment.push('-');
    segment.push_str(&suffix.to_string());

    segment
}

fn sanitize_component(value: &str) -> String {
    value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect()
}
