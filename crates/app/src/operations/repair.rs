//! Recovery repair workflow for replaying committed journals into SQLite.

use anyhow::{Context, Result};
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
    let journal_paths = journal_replay_paths(&report);

    if journal_paths.is_empty() {
        ui.success("No replayable journal findings were found.");
        return Ok(());
    }

    ui.info(format!(
        "Found {} committed journal replay candidate(s).",
        journal_paths.len()
    ));

    if !yes {
        let prompt = format!(
            "Replay {} committed journal(s) into SQLite?",
            journal_paths.len()
        );
        if !ui.confirm(&prompt, false)? {
            ui.notice("Repair aborted.");
            return Ok(());
        }
    }

    let replayed = ui.spinner(
        format!("Replaying {} committed journal(s)...", journal_paths.len()),
        || replay_committed_journals(&journal_paths),
    )?;

    ui.success(format!("Replayed {replayed} committed journal(s)."));
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

fn journal_replay_paths(report: &HealthReport) -> Vec<PathBuf> {
    let mut journal_paths = report
        .recovery_findings
        .iter()
        .filter(|finding| finding.action_group == Some(RecoveryActionGroup::JournalReplay))
        .filter_map(|finding| finding.target_path.as_ref().map(PathBuf::from))
        .collect::<Vec<_>>();

    journal_paths.sort();
    journal_paths.dedup();
    journal_paths
}
