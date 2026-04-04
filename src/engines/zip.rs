use anyhow::Result;
use std::fs;
use std::path::Path;

use super::common::{cleanup_path, extract_zip_archive, replace_directory};

pub fn install(download_path: &Path, install_dir: &Path) -> Result<()> {
    let stage_dir = install_dir.parent().unwrap_or(install_dir).join("staging");

    cleanup_path(&stage_dir)?;
    fs::create_dir_all(&stage_dir)?;

    extract_zip_archive(download_path, &stage_dir)?;
    replace_directory(&stage_dir, install_dir)?;

    Ok(())
}
