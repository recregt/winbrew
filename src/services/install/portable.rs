use anyhow::{Context, Result};

use crate::core::install::{
    begin_install, fail_install, finalize_install, insert_installing_package, source_file_name,
};
use crate::core::network::download_and_verify;

use super::InstallPlan;

pub fn install(
    conn: &rusqlite::Connection,
    context: &InstallPlan,
    on_progress: &impl Fn(u64, u64),
) -> Result<()> {
    begin_install(context)?;

    let install_file_name = source_file_name(&context.source.url)
        .unwrap_or_else(|| format!("{}-{}.exe", context.name, context.package_version));
    let install_file = context.install_dir.join(install_file_name);

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

        std::fs::copy(&context.cache_file, &install_file)
            .context("failed to copy portable package")?;

        finalize_install(conn, context)?;
        Ok(())
    })();

    if result.is_err() {
        fail_install(conn, context);
    }

    result
}
