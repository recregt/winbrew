pub fn version_string() -> String {
    format!(
        "{} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("WINBREW_GIT_HASH")
    )
}