use anyhow::{Result, bail};

use crate::database;
use crate::models::PackageStatus;

pub mod msi;
pub mod msix;
pub mod portable;
pub mod resolve;

pub use crate::core::install::InstallPlan;
pub use resolve::{Resolution, ResolvedInstall};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Progress {
    Downloading { current: u64, total: u64 },
    Status(String),
}

pub fn resolve_plan(name: &str, version: &str) -> Result<InstallPlan> {
    let conn = database::lock_conn()?;
    let manifest = crate::services::fetch_manifest(&conn, name, version)?;

    manifest.validate_download_kind()?;

    crate::core::install::build_plan(name, &manifest)
}

pub fn execute_plan<F>(context: &InstallPlan, mut on_progress: F) -> Result<()>
where
    F: FnMut(Progress),
{
    let conn = database::lock_conn()?;
    if let Some(pkg) = database::get_package(&conn, &context.name)?
        && pkg.status == PackageStatus::Ok
        && pkg.version == context.package_version
    {
        return Ok(());
    }

    on_progress(Progress::Status("Preparing installation...".to_string()));

    match context.source.kind.trim().to_ascii_lowercase().as_str() {
        "portable" => portable::install(&conn, context, &mut |current, total| {
            on_progress(Progress::Downloading { current, total })
        }),
        "msi" => msi::install(&conn, context, &mut |current, total| {
            on_progress(Progress::Downloading { current, total })
        }),
        "msix" => msix::install(&conn, context, &mut |current, total| {
            on_progress(Progress::Downloading { current, total })
        }),
        other => bail!("unsupported download type: {other}"),
    }
}

pub fn install(name: &str, version: &str, on_progress: impl Fn(u64, u64)) -> Result<()> {
    let plan = resolve_plan(name, version)?;
    execute_plan(&plan, |event| {
        if let Progress::Downloading { current, total } = event {
            on_progress(current, total);
        }
    })
}
