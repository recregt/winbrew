use anyhow::Result;
use std::fs;
use std::path::Path;

use crate::core::network::installer_filename;

use super::common::{cleanup_path, extract_zip_archive, replace_directory};

pub fn install(download_path: &Path, install_dir: &Path, installer_url: &str) -> Result<()> {
    let stage_dir = install_dir.parent().unwrap_or(install_dir).join("staging");

    cleanup_path(&stage_dir)?;
    fs::create_dir_all(&stage_dir)?;

    if is_zip_installer(installer_url) {
        extract_zip_archive(download_path, &stage_dir)?;
    } else {
        let file_name = installer_filename(installer_url);
        let target_path = stage_dir.join(file_name);
        fs::copy(download_path, &target_path)?;
    }

    replace_directory(&stage_dir, install_dir)?;

    Ok(())
}

fn is_zip_installer(url: &str) -> bool {
    url.rsplit_once('.')
        .map(|(_, ext)| ext.eq_ignore_ascii_case("zip"))
        .unwrap_or(false)
}
