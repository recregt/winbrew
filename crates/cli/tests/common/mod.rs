use std::path::{Path, PathBuf};
use std::process::{Command, Output};

pub fn test_root() -> tempfile::TempDir {
    tempfile::tempdir().expect("failed to create test root")
}

pub fn init_database(root: &std::path::Path) -> anyhow::Result<()> {
    let config = winbrew_cli::database::Config::load_at(root)?;
    winbrew_cli::database::init(&config.resolved_paths())
}

#[allow(dead_code)]
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("cli crate should live under crates/")
        .to_path_buf()
}

#[allow(dead_code)]
pub fn run_winbrew(root: &Path, args: &[&str]) -> Output {
    Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(repo_root().join("Cargo.toml"))
        .arg("-p")
        .arg("winbrew-bin")
        .arg("--bin")
        .arg("winbrew")
        .arg("--")
        .args(args)
        .env("WINBREW_PATHS_ROOT", root)
        .env("NO_COLOR", "1")
        .current_dir(repo_root())
        .output()
        .expect("failed to run winbrew binary")
}
