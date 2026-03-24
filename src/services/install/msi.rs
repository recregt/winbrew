use anyhow::{Context, Result, bail};
use std::process::Command;

use crate::core::install::InstallTransaction;
use crate::core::network::NetworkSettings;
use crate::core::network::download_and_verify;

use super::InstallPlan;

pub fn install(
    conn: &rusqlite::Connection,
    context: &InstallPlan,
    on_progress: &mut impl FnMut(u64, u64),
) -> Result<()> {
    let tx = InstallTransaction::start(conn, context)?;
    let settings = NetworkSettings::current();

    download_and_verify(
        &settings,
        &context.source.url,
        &context.cache_file,
        &context.source.checksum,
        on_progress,
    )
    .context("download and verification failed")?;

    if context.cache_file.extension().and_then(|ext| ext.to_str()) != Some("msi") {
        bail!(
            "msi installer cache file must have a .msi extension: {}",
            context.cache_file.display()
        );
    }

    let status = Command::new("msiexec")
        .args([
            "/i",
            &context.cache_file.to_string_lossy(),
            "/quiet",
            "/norestart",
        ])
        .status()
        .context("failed to start msiexec")?;

    if !status.success() {
        bail!("msi installer failed with code: {:?}", status.code());
    }

    tx.commit()
}
