//! Recovery repair workflow for replaying committed journals into SQLite.

use anyhow::{Context, Result};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;

use crate::AppContext;
use crate::doctor;
use crate::models::{HealthReport, RecoveryActionGroup};
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
    let file_restore_count = recovery_count(&report, RecoveryActionGroup::FileRestore);
    let reinstall_count = recovery_count(&report, RecoveryActionGroup::Reinstall);

    if journal_paths.is_empty() && orphan_paths.is_empty() {
        ui.success("No supported recovery actions were found.");
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

    if file_restore_count > 0 || reinstall_count > 0 {
        ui.warn(format!(
            "Found {} file restore and {} reinstall candidate(s), but repair does not apply those groups yet.",
            file_restore_count, reinstall_count
        ));
    }

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
