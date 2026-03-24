use anyhow::{Context, Result, bail};
use std::process::Command;

use crate::core::install::{
    begin_install, fail_install, finalize_install, insert_installing_package,
};
use crate::core::network::download_and_verify;

use super::InstallPlan;

pub fn install(
    conn: &rusqlite::Connection,
    context: &InstallPlan,
    on_progress: &mut impl FnMut(u64, u64),
) -> Result<()> {
    begin_install(context)?;

    insert_installing_package(conn, context)?;

    let result = (|| -> Result<()> {
        download_and_verify(
            conn,
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

        finalize_install(conn, context)?;
        Ok(())
    })();

    if result.is_err() {
        fail_install(conn, context);
    }

    result
}
