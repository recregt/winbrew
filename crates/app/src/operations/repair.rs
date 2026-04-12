//! Recovery repair workflow for replaying committed journals into SQLite.

use anyhow::{Context, Result};
use std::path::PathBuf;

use crate::AppContext;
use crate::storage::database;
use winbrew_ui::Ui;

/// Replay all committed journals under the current install root into SQLite.
///
/// This is the low-risk repair path described by the recovery policy. It is
/// intentionally journal-first: only committed journals are replayed, and each
/// replay is applied through the storage transaction boundary so repeated runs
/// converge on the same database state.
pub fn run(ctx: &AppContext, yes: bool) -> Result<()> {
    let mut ui = Ui::new(ctx.ui);
    ui.page_title("Repair");

    let journal_paths = database::JournalReader::committed_paths(&ctx.paths.root)?;
    if journal_paths.is_empty() {
        ui.success("No committed journals found to replay.");
        return Ok(());
    }

    ui.info(format!(
        "Found {} committed journal(s) available for replay.",
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
