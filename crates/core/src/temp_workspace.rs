use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEMP_ROOT_SUFFIX: AtomicUsize = AtomicUsize::new(0);

pub fn build_temp_root(name: &str, version: &str) -> PathBuf {
    temp_root_base().join(temp_root_name(name, version))
}

pub fn temp_root_prefix(name: &str, version: &str) -> String {
    format!(
        "winbrew-install-{}-{}-",
        sanitize_component(name),
        sanitize_component(version)
    )
}

pub fn temp_root_base() -> PathBuf {
    std::env::temp_dir().join("winbrew")
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
