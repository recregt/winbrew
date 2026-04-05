use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

static TEMP_ROOT_SUFFIX: AtomicUsize = AtomicUsize::new(0);

pub fn build_temp_root(name: &str, version: &str) -> PathBuf {
    let sanitized_name = sanitize_component(name);
    let sanitized_version = sanitize_component(version);
    let suffix = TEMP_ROOT_SUFFIX.fetch_add(1, Ordering::Relaxed);
    let mut segment = String::with_capacity(
        "winbrew-install--".len() + sanitized_name.len() + sanitized_version.len() + 20,
    );
    segment.push_str("winbrew-install-");
    segment.push_str(&sanitized_name);
    segment.push('-');
    segment.push_str(&sanitized_version);
    segment.push('-');
    segment.push_str(&std::process::id().to_string());
    segment.push('-');
    segment.push_str(&suffix.to_string());

    std::env::temp_dir().join(segment)
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
