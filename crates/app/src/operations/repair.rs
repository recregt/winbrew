//! Recovery repair workflow for replaying committed journals into SQLite.

use anyhow::{Context, Result};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use crate::AppContext;
use crate::doctor;
use crate::models::{CatalogPackage, HealthReport, RecoveryActionGroup};
use crate::operations::install::{self, InstallObserver, PackageRef};
use crate::operations::remove;
use crate::storage::database;
use winbrew_ui::Ui;

/// Replay journal recovery candidates into SQLite.
///
/// This is the low-risk repair path described by the recovery policy. It asks
/// doctor for replayable recovery findings, then replays only the committed
/// journal targets that doctor has classified as journal-replay candidates.
pub fn run(ctx: &AppContext, yes: bool) -> Result<()> {
    let mut ui = Ui::new(ctx.ui);
    ui.page_title("Repair");

    let report = ui.spinner("Inspecting recovery findings...", || {
        doctor::health_report(ctx)
    })?;
    let journal_paths = recovery_paths(&report, RecoveryActionGroup::JournalReplay);
    let orphan_paths = recovery_paths(&report, RecoveryActionGroup::OrphanCleanup);
    let file_restore_packages = recovery_package_names(
        &report,
        &ctx.paths.packages,
        RecoveryActionGroup::FileRestore,
    );
    let mut reinstall_packages =
        recovery_package_names(&report, &ctx.paths.packages, RecoveryActionGroup::Reinstall);
    reinstall_packages.retain(|package_name| !file_restore_packages.contains(package_name));

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
                "Found {} file restore and {} reinstall candidate(s), but repair does not apply those groups yet.",
                file_restore_count, reinstall_count
            ));
        }
        return Ok(());
    }

    let mut applied = 0usize;

    applied += run_journal_replay_group(&mut ui, yes, &journal_paths)?;
    applied += run_orphan_cleanup_group(&mut ui, yes, &orphan_paths)?;

    applied += run_high_risk_group(
        &mut ui,
        ctx,
        yes,
        RecoveryActionGroup::FileRestore,
        &file_restore_packages,
    )?;
    applied += run_high_risk_group(
        &mut ui,
        ctx,
        yes,
        RecoveryActionGroup::Reinstall,
        &reinstall_packages,
    )?;

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

fn package_name_from_target_path(packages_root: &Path, target_path: &Path) -> Option<String> {
    let relative_path = target_path.strip_prefix(packages_root).ok()?;
    let package_name = relative_path.components().next()?.as_os_str().to_str()?;

    if package_name.is_empty() {
        return None;
    }

    Some(package_name.to_string())
}

fn run_high_risk_group<W: std::io::Write>(
    ui: &mut Ui<W>,
    ctx: &AppContext,
    yes: bool,
    action_group: RecoveryActionGroup,
    package_names: &[String],
) -> Result<usize> {
    if package_names.is_empty() {
        return Ok(0);
    }

    let group_label = match action_group {
        RecoveryActionGroup::FileRestore => "file restore",
        RecoveryActionGroup::Reinstall => "reinstall",
        _ => unreachable!("high-risk group must be file restore or reinstall"),
    };

    ui.info(format!(
        "Found {} {} package candidate(s).",
        package_names.len(),
        group_label
    ));

    let prompt = match action_group {
        RecoveryActionGroup::FileRestore => format!(
            "Restore {} package(s) to repair disk drift?",
            package_names.len()
        ),
        RecoveryActionGroup::Reinstall => {
            format!("Reinstall {} package(s)?", package_names.len())
        }
        _ => unreachable!("high-risk group must be file restore or reinstall"),
    };

    let skipped_message = match action_group {
        RecoveryActionGroup::FileRestore => "Skipped file restore.",
        RecoveryActionGroup::Reinstall => "Skipped reinstall.",
        _ => unreachable!("high-risk group must be file restore or reinstall"),
    };

    if !confirm_group(ui, yes, &prompt, skipped_message)? {
        return Ok(0);
    }

    let repaired = repair_high_risk_packages(ui, ctx, package_names)?;

    ui.success(format!("Repaired {repaired} {group_label} package(s)."));
    Ok(repaired)
}

fn repair_high_risk_packages<W: std::io::Write>(
    ui: &mut Ui<W>,
    ctx: &AppContext,
    package_names: &[String],
) -> Result<usize> {
    let mut repaired = 0usize;
    let conn = database::get_conn()?;

    for package_name in package_names {
        if database::get_package(&conn, package_name)?.is_some() {
            remove::remove(package_name, true).with_context(|| {
                format!("failed to remove package before repair: {package_name}")
            })?;
        }

        let package_ref = PackageRef::parse(package_name)
            .with_context(|| format!("failed to parse package reference '{package_name}'"))?;
        let outcome = {
            let mut observer = RepairInstallObserver { ui };
            install::run(ctx, package_ref, false, &mut observer)
                .with_context(|| format!("failed to reinstall package '{package_name}'"))?
        };

        ui.success(format!(
            "Repaired {} {}.",
            outcome.result.name, outcome.result.version
        ));
        repaired += 1;
    }

    Ok(repaired)
}

struct RepairInstallObserver<'a, W: std::io::Write> {
    ui: &'a mut Ui<W>,
}

impl<'a, W: std::io::Write> InstallObserver for RepairInstallObserver<'a, W> {
    fn choose_package(&mut self, query: &str, matches: &[CatalogPackage]) -> anyhow::Result<usize> {
        let choices = matches
            .iter()
            .map(format_catalog_choice)
            .collect::<Vec<_>>();

        self.ui.select_index(
            &format!("Multiple packages matched '{query}'. Choose one:"),
            &choices,
        )
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
