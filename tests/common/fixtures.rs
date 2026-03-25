#![allow(dead_code)]

use anyhow::{Context, Result};
use std::fs;
use std::path::{Path, PathBuf};

pub fn winget_fixture(name: &str) -> Result<String> {
    fs::read_to_string(winget_fixture_path(name))
        .with_context(|| format!("failed to read winget fixture: {name}"))
}

pub fn winget_fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("winget")
        .join(name)
}

pub fn init_database_root(root: &Path) -> Result<()> {
    winbrew::database::config_set("paths.root", &root.to_string_lossy())?;
    Ok(())
}
