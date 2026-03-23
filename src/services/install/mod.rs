use anyhow::{Result, bail};

use crate::database;
use crate::models::PackageStatus;

pub mod msi;
pub mod portable;

pub use crate::core::install::InstallPlan;

pub fn install(name: &str, version: &str, on_progress: impl Fn(u64, u64)) -> Result<()> {
    let conn = database::lock_conn()?;
    let manifest = crate::services::fetch_manifest(&conn, name, version)?;

    manifest.validate_download_kind()?;

    let context = crate::core::install::build_plan(name, &manifest)?;
    if let Some(pkg) = database::get_package(&conn, name)?
        && pkg.status == PackageStatus::Ok
        && pkg.version == context.package_version
    {
        return Ok(());
    }

    match context.source.kind.trim().to_ascii_lowercase().as_str() {
        "portable" => portable::install(&conn, &context, &on_progress),
        "msi" => msi::install(&conn, &context, &on_progress),
        other => bail!("unsupported download type: {other}"),
    }
}
