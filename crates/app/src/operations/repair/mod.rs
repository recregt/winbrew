//! Recovery repair helpers for replaying committed journals, cleaning orphans,
//! and resolving high-risk recovery candidates.

mod cleanup;
mod plan;
mod replay;
mod resolution;
mod restore;

pub use cleanup::cleanup_orphan_install_dirs;
pub use plan::{FileRestorePackage, RepairPlan, build_repair_plan};
pub use replay::{
    JournalCommandResolutionStatus, JournalReplaySummary, JournalReplayTarget,
    prepare_journal_replay_targets, replay_committed_journals, replay_prepared_journal_targets,
    summarize_journal_replay_targets,
};
pub use resolution::{
    FileRestoreReinstallTarget, FileRestoreResolution, ResolvedFileRestoreTarget,
    reinstall_package, resolve_file_restore_target, resolve_repair_catalog_package,
};
pub use restore::restore_file_restore_target;
