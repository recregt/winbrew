pub fn test_root() -> tempfile::TempDir {
    tempfile::tempdir().expect("failed to create test root")
}

pub fn init_database(root: &std::path::Path) -> anyhow::Result<()> {
    let config = winbrew_cli::database::Config::load_at(root)?;
    winbrew_cli::database::init(&config.resolved_paths())
}
