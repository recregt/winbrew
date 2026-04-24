use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use tracing::warn;

use crate::core::paths::install_root_from_package_dir;
use crate::database;
use crate::models::domains::command_resolution::ResolverResult;
use crate::operations::install;
use crate::operations::shims;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JournalCommandResolutionStatus {
    Unknown,
    Fresh,
    Stale {
        committed_fingerprint: String,
        current_fingerprint: String,
    },
}

#[derive(Debug, Clone)]
pub struct JournalReplayTarget {
    pub journal_path: PathBuf,
    pub committed: database::CommittedJournalPackage,
    pub command_resolution_status: JournalCommandResolutionStatus,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JournalReplaySummary {
    pub total: usize,
    pub fresh: usize,
    pub stale: usize,
    pub unknown: usize,
}

pub fn replay_committed_journals(journal_paths: &[PathBuf]) -> Result<usize> {
    let targets = prepare_journal_replay_targets(journal_paths)?;
    replay_prepared_journal_targets(&targets)
}

pub fn prepare_journal_replay_targets(
    journal_paths: &[PathBuf],
) -> Result<Vec<JournalReplayTarget>> {
    let catalog_conn = match database::get_catalog_conn() {
        Ok(conn) => Some(conn),
        Err(err) => {
            warn!(
                error = %err,
                "failed to open catalog database for repair command resolution comparison"
            );
            None
        }
    };
    let mut targets = Vec::with_capacity(journal_paths.len());

    for journal_path in journal_paths {
        let committed = database::JournalReader::read_committed_package(journal_path)
            .with_context(|| {
                format!(
                    "failed to parse committed journal at {}",
                    journal_path.display()
                )
            })?;

        if committed.command_resolution.is_none() {
            bail!(
                "committed journal at {} is missing command resolution metadata",
                journal_path.display()
            );
        }

        let current_resolution = catalog_conn
            .as_ref()
            .and_then(|conn| current_command_resolution(conn, &committed.package.name));

        let command_resolution_status = classify_journal_command_resolution_status(
            committed.command_resolution.as_ref(),
            current_resolution,
        );

        if let JournalCommandResolutionStatus::Stale {
            committed_fingerprint,
            current_fingerprint,
        } = &command_resolution_status
        {
            warn!(
                package = committed.package.name.as_str(),
                committed_fingerprint = committed_fingerprint.as_str(),
                current_fingerprint = current_fingerprint.as_str(),
                "committed journal command resolution fingerprint differs from current catalog metadata"
            );
        }

        targets.push(JournalReplayTarget {
            journal_path: journal_path.clone(),
            committed,
            command_resolution_status,
        });
    }

    Ok(targets)
}

pub fn replay_prepared_journal_targets(targets: &[JournalReplayTarget]) -> Result<usize> {
    let mut conn = database::get_conn()?;
    let mut replayed = 0usize;

    for target in targets {
        let committed = &target.committed;
        let previous_commands = database::list_commands_for_package(&conn, &committed.package.name)
            .unwrap_or_else(|err| {
                warn!(
                    package = committed.package.name.as_str(),
                    error = %err,
                    "failed to read existing package commands before replay"
                );
                Vec::new()
            });
        database::replay_committed_journal(&mut conn, committed).with_context(|| {
            format!(
                "failed to replay committed journal at {}",
                target.journal_path.display()
            )
        })?;
        let shims_root =
            install_root_from_package_dir(Path::new(&committed.package.install_dir)).join("shims");
        let desired_commands = journal_commands(committed);
        let empty_paths: &[String] = &[];
        let target_paths = committed.bin.as_deref().unwrap_or(empty_paths);

        if let Err(err) = shims::publish_shims_for_install_dir(
            &shims_root,
            Path::new(&committed.package.install_dir),
            desired_commands,
            target_paths,
        ) {
            warn!(
                package = committed.package.name.as_str(),
                error = %err,
                "failed to publish package shims during repair replay"
            );
        } else {
            let desired_commands = desired_commands.iter().cloned().collect::<BTreeSet<_>>();
            let stale_commands = previous_commands
                .into_iter()
                .filter(|command| !desired_commands.contains(command))
                .collect::<Vec<_>>();

            if !stale_commands.is_empty()
                && let Err(err) = shims::remove_shim_files(&shims_root, &stale_commands)
            {
                warn!(
                    package = committed.package.name.as_str(),
                    error = %err,
                    "failed to remove stale package shims during repair replay"
                );
            }
        }
        replayed += 1;
    }

    Ok(replayed)
}

pub fn summarize_journal_replay_targets(targets: &[JournalReplayTarget]) -> JournalReplaySummary {
    let mut summary = JournalReplaySummary {
        total: targets.len(),
        ..JournalReplaySummary::default()
    };

    for target in targets {
        match target.command_resolution_status {
            JournalCommandResolutionStatus::Fresh => summary.fresh += 1,
            JournalCommandResolutionStatus::Stale { .. } => summary.stale += 1,
            JournalCommandResolutionStatus::Unknown => summary.unknown += 1,
        }
    }

    summary
}

fn journal_commands(committed: &database::CommittedJournalPackage) -> &[String] {
    match committed.command_resolution.as_ref() {
        Some(ResolverResult::Resolved { commands, .. }) => commands.as_slice(),
        Some(ResolverResult::Unresolved { .. }) | None => &[],
    }
}

fn current_command_resolution(
    catalog_conn: &database::DbConnection,
    package_id: &str,
) -> Option<ResolverResult> {
    let package = match database::get_package_by_id(catalog_conn, package_id) {
        Ok(Some(package)) => package,
        Ok(None) => return None,
        Err(err) => {
            warn!(
                package = package_id,
                error = %err,
                "failed to read catalog package for repair command resolution comparison"
            );
            return None;
        }
    };

    let installers = match database::get_installers(catalog_conn, package.id.as_str()) {
        Ok(installers) => installers,
        Err(err) => {
            warn!(
                package = package_id,
                error = %err,
                "failed to read catalog installers for repair command resolution comparison"
            );
            return None;
        }
    };

    let selection_context = crate::catalog::SelectionContext::new(
        crate::windows::host_profile(),
        crate::windows::is_elevated(),
    );
    let installer = match install::types::select_installer(&installers, selection_context) {
        Ok(installer) => installer,
        Err(err) => {
            warn!(
                package = package_id,
                error = %err,
                "failed to select catalog installer for repair command resolution comparison"
            );
            return None;
        }
    };

    match crate::models::domains::command_resolution::resolve_command_exposure(&package, &installer)
    {
        Ok(resolution) => Some(resolution),
        Err(err) => {
            warn!(
                package = package_id,
                error = %err,
                "failed to resolve current command exposure for repair comparison"
            );
            None
        }
    }
}

pub(crate) fn classify_journal_command_resolution_status(
    committed: Option<&ResolverResult>,
    current: Option<ResolverResult>,
) -> JournalCommandResolutionStatus {
    let Some(committed_resolution) = committed else {
        return JournalCommandResolutionStatus::Unknown;
    };

    let ResolverResult::Resolved {
        catalog_fingerprint: committed_fingerprint,
        ..
    } = committed_resolution
    else {
        return JournalCommandResolutionStatus::Unknown;
    };

    let Some(current_resolution) = current.as_ref() else {
        return JournalCommandResolutionStatus::Unknown;
    };

    let ResolverResult::Resolved {
        catalog_fingerprint: current_fingerprint,
        ..
    } = current_resolution
    else {
        return JournalCommandResolutionStatus::Unknown;
    };

    if !command_resolution_is_stale(committed_resolution, current_resolution) {
        JournalCommandResolutionStatus::Fresh
    } else {
        JournalCommandResolutionStatus::Stale {
            committed_fingerprint: committed_fingerprint.clone(),
            current_fingerprint: current_fingerprint.clone(),
        }
    }
}

pub(crate) fn command_resolution_is_stale(
    committed: &ResolverResult,
    current: &ResolverResult,
) -> bool {
    match (committed, current) {
        (
            ResolverResult::Resolved {
                catalog_fingerprint: committed_fingerprint,
                ..
            },
            ResolverResult::Resolved {
                catalog_fingerprint: current_fingerprint,
                ..
            },
        ) => committed_fingerprint != current_fingerprint,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        JournalCommandResolutionStatus, JournalReplayTarget,
        classify_journal_command_resolution_status, command_resolution_is_stale,
        summarize_journal_replay_targets,
    };
    use crate::models::domains::command_resolution::{
        CommandSource, Confidence, ResolverResult, VersionScope,
    };
    use crate::models::domains::install::{EngineKind, InstallerType};
    use crate::models::domains::installed::{InstalledPackage, PackageStatus};
    use crate::models::domains::shared::DeploymentKind;
    use std::path::PathBuf;

    fn test_committed_package() -> crate::database::CommittedJournalPackage {
        crate::database::CommittedJournalPackage {
            journal_path: PathBuf::from("C:/winbrew/pkgdb/Contoso.App/journal.jsonl"),
            entries: Vec::new(),
            package: InstalledPackage {
                name: "Contoso.App".to_string(),
                version: "1.0.0".to_string(),
                kind: InstallerType::Portable,
                deployment_kind: DeploymentKind::Portable,
                engine_kind: EngineKind::Portable,
                engine_metadata: None,
                install_dir: "C:/winbrew/packages/Contoso.App".to_string(),
                dependencies: Vec::new(),
                status: PackageStatus::Ok,
                installed_at: "2026-04-12T00:00:00Z".to_string(),
            },
            commands: Some(vec!["contoso".to_string()]),
            bin: Some(vec!["bin/tool.exe".to_string()]),
            command_resolution: Some(ResolverResult::Resolved {
                commands: vec!["contoso".to_string()],
                confidence: Confidence::High,
                sources: vec![CommandSource::PackageLevel],
                version_scope: VersionScope::Specific("1.0.0".to_string()),
                catalog_fingerprint: "sha256:deadbeef".to_string(),
            }),
        }
    }

    fn test_journal_target(status: JournalCommandResolutionStatus) -> JournalReplayTarget {
        JournalReplayTarget {
            journal_path: PathBuf::from("C:/winbrew/pkgdb/Contoso.App/journal.jsonl"),
            committed: test_committed_package(),
            command_resolution_status: status,
        }
    }

    #[test]
    fn command_resolution_is_stale_when_fingerprints_differ() {
        let committed = ResolverResult::Resolved {
            commands: vec!["contoso".to_string()],
            confidence: Confidence::High,
            sources: vec![CommandSource::PackageLevel],
            version_scope: VersionScope::Specific("1.0.0".to_string()),
            catalog_fingerprint: "sha256:deadbeef".to_string(),
        };
        let current = ResolverResult::Resolved {
            commands: vec!["contoso".to_string()],
            confidence: Confidence::High,
            sources: vec![CommandSource::PackageLevel],
            version_scope: VersionScope::Specific("1.0.0".to_string()),
            catalog_fingerprint: "sha256:cafebabe".to_string(),
        };

        assert!(command_resolution_is_stale(&committed, &current));
    }

    #[test]
    fn classify_journal_command_resolution_status_tracks_fresh_stale_and_unknown_states() {
        let committed = ResolverResult::Resolved {
            commands: vec!["contoso".to_string()],
            confidence: Confidence::High,
            sources: vec![CommandSource::PackageLevel],
            version_scope: VersionScope::Specific("1.0.0".to_string()),
            catalog_fingerprint: "sha256:deadbeef".to_string(),
        };
        let current = ResolverResult::Resolved {
            commands: vec!["contoso".to_string()],
            confidence: Confidence::High,
            sources: vec![CommandSource::PackageLevel],
            version_scope: VersionScope::Specific("1.0.0".to_string()),
            catalog_fingerprint: "sha256:deadbeef".to_string(),
        };
        let stale = ResolverResult::Resolved {
            commands: vec!["contoso".to_string()],
            confidence: Confidence::High,
            sources: vec![CommandSource::PackageLevel],
            version_scope: VersionScope::Specific("1.0.0".to_string()),
            catalog_fingerprint: "sha256:cafebabe".to_string(),
        };

        assert!(matches!(
            classify_journal_command_resolution_status(Some(&committed), Some(current)),
            JournalCommandResolutionStatus::Fresh
        ));
        assert!(matches!(
            classify_journal_command_resolution_status(Some(&committed), Some(stale)),
            JournalCommandResolutionStatus::Stale {
                committed_fingerprint,
                current_fingerprint,
            } if committed_fingerprint == "sha256:deadbeef" && current_fingerprint == "sha256:cafebabe"
        ));
        assert!(matches!(
            classify_journal_command_resolution_status(None, None),
            JournalCommandResolutionStatus::Unknown
        ));
    }

    #[test]
    fn summarize_journal_replay_targets_counts_statuses() {
        let summary = summarize_journal_replay_targets(&[
            test_journal_target(JournalCommandResolutionStatus::Fresh),
            test_journal_target(JournalCommandResolutionStatus::Stale {
                committed_fingerprint: "sha256:deadbeef".to_string(),
                current_fingerprint: "sha256:cafebabe".to_string(),
            }),
            test_journal_target(JournalCommandResolutionStatus::Unknown),
        ]);

        assert_eq!(summary.total, 3);
        assert_eq!(summary.fresh, 1);
        assert_eq!(summary.stale, 1);
        assert_eq!(summary.unknown, 1);
    }
}
