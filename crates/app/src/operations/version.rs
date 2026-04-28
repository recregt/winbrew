pub fn version_string() -> String {
    format!(
        "{} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("WINBREW_GIT_HASH")
    )
}

pub fn package_version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
