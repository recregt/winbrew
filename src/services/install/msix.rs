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

    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            &format!(
                "Add-AppxPackage -Path '{}' -ForceApplicationShutdown",
                context.cache_file.display()
            ),
        ])
        .status()
        .context("failed to start PowerShell")?;

    if !status.success() {
        bail!("msix installer failed with code: {:?}", status.code());
    }

    tx.commit()
}
