use std::path::PathBuf;

pub fn build_temp_root(name: &str, version: &str) -> PathBuf {
    let sanitized_name = sanitize_component(name);
    let sanitized_version = sanitize_component(version);
    std::env::temp_dir().join(format!(
        "winbrew-install-{sanitized_name}-{sanitized_version}-{}",
        std::process::id()
    ))
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
