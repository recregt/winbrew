//! Recovery repair workflow for replaying committed journals, cleaning orphans,
//! and handling high-risk recovery candidates.

use anyhow::{Context, Result};
use std::collections::BTreeMap;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::AppContext;
use crate::catalog;
use crate::core::{fs::cleanup_path, temp_workspace};
use crate::doctor;
use crate::engines;
use crate::models::{CatalogInstaller, CatalogPackage, HealthReport, Package, RecoveryActionGroup};
use crate::operations::install::{self, InstallObserver, PackageRef};
use crate::operations::remove;
use crate::storage::database;
use winbrew_ui::Ui;

/// Apply recovery candidates from the doctor report.
///
/// This path starts with the low-risk journal replay and orphan cleanup groups,
/// then handles high-risk file restore and reinstall candidates one package at
/// a time behind explicit confirmation.
pub fn run(ctx: &AppContext, yes: bool) -> Result<()> {
    let mut ui = Ui::new(ctx.ui);
    ui.page_title("Repair");

    let report = ui.spinner("Inspecting recovery findings...", || {
        doctor::health_report(ctx)
    })?;
    let journal_paths = recovery_paths(&report, RecoveryActionGroup::JournalReplay);
    let orphan_paths = recovery_paths(&report, RecoveryActionGroup::OrphanCleanup);
    let file_restore_packages = recovery_file_restore_packages(
        &report,
        &ctx.paths.packages,
        RecoveryActionGroup::FileRestore,
    );
    let mut reinstall_packages =
        recovery_package_names(&report, &ctx.paths.packages, RecoveryActionGroup::Reinstall);
    reinstall_packages.retain(|package_name| {
        !file_restore_packages
            .iter()
            .any(|candidate| candidate.name == *package_name)
    });

    if journal_paths.is_empty()
        && orphan_paths.is_empty()
        && file_restore_packages.is_empty()
        && reinstall_packages.is_empty()
    {
        ui.success("No supported recovery actions were found.");
        let file_restore_count = recovery_count(&report, RecoveryActionGroup::FileRestore);
        let reinstall_count = recovery_count(&report, RecoveryActionGroup::Reinstall);
        if file_restore_count > 0 || reinstall_count > 0 {
            ui.warn(format!(
                "Found {} file restore and {} reinstall finding(s), but no package targets were derived.",
                file_restore_count, reinstall_count
            ));
        }
        return Ok(());
    }

    let mut applied = 0usize;

    applied += run_journal_replay_group(&mut ui, yes, &journal_paths)?;
    applied += run_orphan_cleanup_group(&mut ui, yes, &orphan_paths)?;

    applied += run_file_restore_group(&mut ui, ctx, yes, &file_restore_packages)?;
    applied += run_reinstall_group(&mut ui, ctx, yes, &reinstall_packages)?;

    if applied == 0 {
        ui.notice("No recovery actions were applied.");
    }

    Ok(())
}

pub(crate) fn replay_committed_journals(journal_paths: &[PathBuf]) -> Result<usize> {
    let mut conn = database::get_conn()?;
    let mut replayed = 0usize;

    for journal_path in journal_paths {
        let committed = database::JournalReader::read_committed_package(journal_path)
            .with_context(|| {
                format!(
                    "failed to parse committed journal at {}",
                    journal_path.display()
                )
            })?;
        database::replay_committed_journal(&mut conn, &committed).with_context(|| {
            format!(
                "failed to replay committed journal at {}",
                journal_path.display()
            )
        })?;
        replayed += 1;
    }

    Ok(replayed)
}

pub(crate) fn cleanup_orphan_install_dirs(orphan_paths: &[PathBuf]) -> Result<usize> {
    let mut removed = 0usize;

    for orphan_path in orphan_paths {
        match fs::remove_dir_all(orphan_path) {
            Ok(()) => {
                removed += 1;
            }
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
            Err(err) => {
                return Err(err).with_context(|| {
                    format!(
                        "failed to remove orphan install directory at {}",
                        orphan_path.display()
                    )
                });
            }
        }
    }

    Ok(removed)
}

fn run_journal_replay_group<W: std::io::Write>(
    ui: &mut Ui<W>,
    yes: bool,
    journal_paths: &[PathBuf],
) -> Result<usize> {
    if journal_paths.is_empty() {
        return Ok(0);
    }

    ui.info(format!(
        "Found {} committed journal replay candidate(s).",
        journal_paths.len()
    ));

    if !confirm_group(
        ui,
        yes,
        &format!(
            "Replay {} committed journal(s) into SQLite?",
            journal_paths.len()
        ),
        "Skipped journal replay.",
    )? {
        return Ok(0);
    }

    let replayed = ui.spinner(
        format!("Replaying {} committed journal(s)...", journal_paths.len()),
        || replay_committed_journals(journal_paths),
    )?;

    ui.success(format!("Replayed {replayed} committed journal(s)."));
    Ok(replayed)
}

fn run_orphan_cleanup_group<W: std::io::Write>(
    ui: &mut Ui<W>,
    yes: bool,
    orphan_paths: &[PathBuf],
) -> Result<usize> {
    if orphan_paths.is_empty() {
        return Ok(0);
    }

    ui.info(format!(
        "Found {} orphan install directory candidate(s).",
        orphan_paths.len()
    ));

    if !confirm_group(
        ui,
        yes,
        &format!(
            "Remove {} orphan install director{}?",
            orphan_paths.len(),
            if orphan_paths.len() == 1 { "y" } else { "ies" }
        ),
        "Skipped orphan cleanup.",
    )? {
        return Ok(0);
    }

    let removed = ui.spinner(
        format!(
            "Removing {} orphan install director{}...",
            orphan_paths.len(),
            if orphan_paths.len() == 1 { "y" } else { "ies" }
        ),
        || cleanup_orphan_install_dirs(orphan_paths),
    )?;

    ui.success(format!(
        "Removed {removed} orphan install director{}.",
        if removed == 1 { "y" } else { "ies" }
    ));
    Ok(removed)
}

fn confirm_group<W: std::io::Write>(
    ui: &mut Ui<W>,
    yes: bool,
    prompt: &str,
    skipped_message: &str,
) -> Result<bool> {
    if yes {
        return Ok(true);
    }

    if ui.confirm(prompt, false)? {
        return Ok(true);
    }

    ui.notice(skipped_message);
    Ok(false)
}

fn recovery_paths(report: &HealthReport, action_group: RecoveryActionGroup) -> Vec<PathBuf> {
    let mut paths = report
        .recovery_findings
        .iter()
        .filter(|finding| finding.action_group == Some(action_group))
        .filter_map(|finding| finding.target_path.as_ref().map(PathBuf::from))
        .collect::<Vec<_>>();

    paths.sort();
    paths.dedup();
    paths
}

fn recovery_count(report: &HealthReport, action_group: RecoveryActionGroup) -> usize {
    report
        .recovery_findings
        .iter()
        .filter(|finding| finding.action_group == Some(action_group))
        .count()
}

fn recovery_package_names(
    report: &HealthReport,
    packages_root: &Path,
    action_group: RecoveryActionGroup,
) -> Vec<String> {
    let mut package_names = report
        .recovery_findings
        .iter()
        .filter(|finding| finding.action_group == Some(action_group))
        .filter_map(|finding| {
            finding.target_path.as_deref().and_then(|target_path| {
                package_name_from_target_path(packages_root, Path::new(target_path))
            })
        })
        .collect::<Vec<_>>();

    package_names.sort_unstable();
    package_names.dedup();
    package_names
}

#[derive(Debug)]
struct FileRestorePackage {
    name: String,
    target_paths: Vec<PathBuf>,
}

fn recovery_file_restore_packages(
    report: &HealthReport,
    packages_root: &Path,
    action_group: RecoveryActionGroup,
) -> Vec<FileRestorePackage> {
    let mut package_targets = BTreeMap::<String, Vec<PathBuf>>::new();

    for finding in report
        .recovery_findings
        .iter()
        .filter(|finding| finding.action_group == Some(action_group))
    {
        let Some(target_path) = finding.target_path.as_deref() else {
            continue;
        };

        let Some(package_name) =
            package_name_from_target_path(packages_root, Path::new(target_path))
        else {
            continue;
        };

        package_targets
            .entry(package_name)
            .or_default()
            .push(PathBuf::from(target_path));
    }

    package_targets
        .into_iter()
        .map(|(name, mut target_paths)| {
            target_paths.sort();
            target_paths.dedup();

            FileRestorePackage { name, target_paths }
        })
        .collect()
}

fn package_name_from_target_path(packages_root: &Path, target_path: &Path) -> Option<String> {
    let relative_path = target_path.strip_prefix(packages_root).ok()?;
    let package_name = relative_path.components().next()?.as_os_str().to_str()?;

    if package_name.is_empty() {
        return None;
    }

    Some(package_name.to_string())
}

fn run_file_restore_group<W: std::io::Write>(
    ui: &mut Ui<W>,
    ctx: &AppContext,
    yes: bool,
    package_targets: &[FileRestorePackage],
) -> Result<usize> {
    if package_targets.is_empty() {
        return Ok(0);
    }

    ui.info(format!(
        "Found {} file restore package candidate(s).",
        package_targets.len()
    ));

    let mut repaired = 0usize;

    for package_target in package_targets {
        let target_count = package_target.target_paths.len();

        if !confirm_group(
            ui,
            yes,
            &format!(
                "Restore {} file{} for {}?",
                target_count,
                if target_count == 1 { "" } else { "s" },
                package_target.name
            ),
            &format!("Skipped file restore for {}.", package_target.name),
        )? {
            continue;
        }

        repair_file_restore_package(ui, ctx, &package_target.name, &package_target.target_paths)?;
        repaired += 1;
    }

    Ok(repaired)
}

fn run_reinstall_group<W: std::io::Write>(
    ui: &mut Ui<W>,
    ctx: &AppContext,
    yes: bool,
    package_names: &[String],
) -> Result<usize> {
    if package_names.is_empty() {
        return Ok(0);
    }

    ui.info(format!(
        "Found {} reinstall package candidate(s).",
        package_names.len()
    ));

    let mut repaired = 0usize;

    for package_name in package_names {
        if !confirm_group(
            ui,
            yes,
            &format!("Reinstall {package_name}?"),
            &format!("Skipped reinstall for {package_name}."),
        )? {
            continue;
        }

        let outcome = repair_reinstall_package(ui, ctx, package_name)?;

        ui.success(format!(
            "Repaired {} {}.",
            outcome.result.name, outcome.result.version
        ));
        repaired += 1;
    }

    Ok(repaired)
}

fn repair_file_restore_package<W: std::io::Write>(
    ui: &mut Ui<W>,
    ctx: &AppContext,
    package_name: &str,
    target_paths: &[PathBuf],
) -> Result<usize> {
    let package_ref = PackageRef::parse(package_name)
        .with_context(|| format!("failed to parse package reference '{package_name}'"))?;
    let catalog_conn = crate::storage::get_catalog_conn()?;
    let package =
        catalog::resolve_catalog_package_ref(&catalog_conn, &package_ref, |query, matches| {
            choose_catalog_package(ui, query, matches)
        })?;

    let conn = database::get_conn()?;
    let installed_package = database::get_package(&conn, package_name)?
        .with_context(|| format!("package '{package_name}' is not installed"))?;

    let installers = crate::storage::get_installers(&catalog_conn, &package.id)?;
    let installer = install::types::select_installer(&installers)?;
    let engine = engines::resolve_engine_for_installer(&installer)?;

    if installed_package.version != package.version.to_string() {
        ui.notice(format!(
            "Catalog version {} differs from installed version {}; reinstalling {package_name} instead.",
            package.version, installed_package.version
        ));

        let outcome = repair_reinstall_package(ui, ctx, package_name)?;
        ui.success(format!(
            "Repaired {} {}.",
            outcome.result.name, outcome.result.version
        ));
        return Ok(1);
    }

    let restored = ui.spinner(
        format!(
            "Restoring {} file{} for {}...",
            target_paths.len(),
            if target_paths.len() == 1 { "" } else { "s" },
            package_name
        ),
        || {
            restore_package_files(
                &package,
                &installer,
                engine,
                &installed_package,
                target_paths,
            )
        },
    )?;

    ui.success(format!(
        "Restored {} file{} for {}.",
        restored,
        if restored == 1 { "" } else { "s" },
        package_name
    ));

    Ok(restored)
}

fn repair_reinstall_package<W: std::io::Write>(
    ui: &mut Ui<W>,
    ctx: &AppContext,
    package_name: &str,
) -> Result<install::InstallOutcome> {
    let conn = database::get_conn()?;

    if database::get_package(&conn, package_name)?.is_some() {
        remove::remove(package_name, true)
            .with_context(|| format!("failed to remove package before repair: {package_name}"))?;
    }

    let package_ref = PackageRef::parse(package_name)
        .with_context(|| format!("failed to parse package reference '{package_name}'"))?;
    let mut observer = RepairInstallObserver { ui };
    install::run(ctx, package_ref, false, &mut observer)
        .with_context(|| format!("failed to reinstall package '{package_name}'"))
}

fn restore_package_files(
    package: &CatalogPackage,
    installer: &CatalogInstaller,
    engine: crate::engines::EngineKind,
    installed_package: &Package,
    target_paths: &[PathBuf],
) -> Result<usize> {
    let temp_root = temp_workspace::build_temp_root(&package.name, &package.version.to_string());
    cleanup_path(&temp_root)?;
    fs::create_dir_all(&temp_root)?;

    let result = (|| -> Result<usize> {
        let stage_dir = temp_root.join("stage");
        let client = install::download::build_client()?;

        let _ = install::flow::perform_install(install::flow::InstallRequest {
            client: &client,
            engine,
            installer,
            package_name: &package.name,
            temp_root: &temp_root,
            install_dir: &stage_dir,
            ignore_checksum_security: false,
            on_start: |_| {},
            on_progress: |_| {},
        })?;

        restore_target_files(
            &stage_dir,
            Path::new(&installed_package.install_dir),
            target_paths,
        )
    })();

    let _ = cleanup_path(&temp_root);

    result
}

fn restore_target_files(
    stage_dir: &Path,
    install_dir: &Path,
    target_paths: &[PathBuf],
) -> Result<usize> {
    let mut restored = 0usize;

    for target_path in target_paths {
        let relative_path = target_path.strip_prefix(install_dir).with_context(|| {
            format!(
                "failed to derive restored file path for {} from {}",
                target_path.display(),
                install_dir.display()
            )
        })?;
        let source_path = stage_dir.join(relative_path);

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to prepare parent directory for {}",
                    target_path.display()
                )
            })?;
        }

        fs::copy(&source_path, target_path).with_context(|| {
            format!(
                "failed to restore file {} from staged package",
                target_path.display()
            )
        })?;

        restored += 1;
    }

    Ok(restored)
}

fn choose_catalog_package<W: std::io::Write>(
    ui: &mut Ui<W>,
    query: &str,
    matches: &[CatalogPackage],
) -> Result<usize> {
    let choices = matches
        .iter()
        .map(format_catalog_choice)
        .collect::<Vec<_>>();

    ui.select_index(
        &format!("Multiple packages matched '{query}'. Choose one:"),
        &choices,
    )
}

struct RepairInstallObserver<'a, W: std::io::Write> {
    ui: &'a mut Ui<W>,
}

impl<'a, W: std::io::Write> InstallObserver for RepairInstallObserver<'a, W> {
    fn choose_package(&mut self, query: &str, matches: &[CatalogPackage]) -> anyhow::Result<usize> {
        choose_catalog_package(self.ui, query, matches)
    }

    fn on_start(&mut self, _total_bytes: Option<u64>) {}

    fn on_progress(&mut self, _downloaded_bytes: u64) {}
}

fn format_catalog_choice(pkg: &CatalogPackage) -> String {
    let mut label = String::with_capacity(128);
    label.push_str(&pkg.name);
    label.push(' ');
    label.push_str(&pkg.version.to_string());

    if let Some(publisher) = pkg
        .publisher
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        label.push_str(" - ");
        label.push_str(publisher);
    }

    if let Some(description) = pkg
        .description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        label.push_str(" (");
        label.push_str(description);
        label.push(')');
    }

    label
}

#[cfg(test)]
mod tests {
    use super::restore_target_files;
    use anyhow::Result;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn restore_target_files_copies_staged_content() -> Result<()> {
        let root = tempdir()?;
        let stage_dir = root.path().join("stage");
        let install_dir = root.path().join("packages").join("Contoso.App");
        let target_path = install_dir.join("bin").join("tool.exe");
        let staged_path = stage_dir.join("bin").join("tool.exe");

        fs::create_dir_all(staged_path.parent().expect("stage parent"))?;
        fs::create_dir_all(target_path.parent().expect("target parent"))?;
        fs::write(&staged_path, b"restored-binary")?;

        let restored = restore_target_files(&stage_dir, &install_dir, &[target_path.clone()])?;

        assert_eq!(restored, 1);
        assert_eq!(fs::read(&target_path)?, b"restored-binary");

        Ok(())
    }
}
