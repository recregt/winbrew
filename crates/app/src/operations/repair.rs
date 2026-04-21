//! Recovery repair helpers for replaying committed journals, cleaning orphans,
//! and resolving high-risk recovery candidates.

use anyhow::{Context, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use tracing::warn;

use crate::AppContext;
use crate::catalog;
use crate::core::{
    fs::cleanup_path, network::installer_filename, paths::install_root_from_package_dir,
    temp_workspace,
};
use crate::database;
use crate::engines::{self, EngineKind};
use crate::models::catalog::{CatalogInstaller, CatalogPackage};
use crate::models::domains::command_resolution::{ResolverResult, resolve_command_exposure};
use crate::models::domains::installed::InstalledPackage;
use crate::models::domains::package::{PackageId, PackageRef};
use crate::models::domains::reporting::{HealthReport, RecoveryActionGroup};
use crate::operations::install::{self, InstallObserver};
use crate::operations::remove;
use crate::operations::shims;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRestorePackage {
    pub name: String,
    pub target_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepairPlan {
    pub journal_paths: Vec<PathBuf>,
    pub orphan_paths: Vec<PathBuf>,
    pub file_restore_packages: Vec<FileRestorePackage>,
    pub reinstall_packages: Vec<String>,
    pub file_restore_count: usize,
    pub reinstall_count: usize,
}

impl RepairPlan {
    pub fn is_empty(&self) -> bool {
        self.journal_paths.is_empty()
            && self.orphan_paths.is_empty()
            && self.file_restore_packages.is_empty()
            && self.reinstall_packages.is_empty()
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedFileRestoreTarget {
    pub package: CatalogPackage,
    pub installer: CatalogInstaller,
    pub engine: EngineKind,
    pub installed_package: InstalledPackage,
}

#[derive(Debug, Clone)]
pub struct FileRestoreReinstallTarget {
    pub catalog_package: CatalogPackage,
    pub installed_version: String,
}

#[derive(Debug, Clone)]
pub enum FileRestoreResolution {
    Restore(Box<ResolvedFileRestoreTarget>),
    Reinstall(Box<FileRestoreReinstallTarget>),
}

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

/// Build the grouped recovery plan from a health report.
pub fn build_repair_plan(report: &HealthReport, packages_root: &Path) -> RepairPlan {
    let journal_paths = recovery_paths(report, RecoveryActionGroup::JournalReplay);
    let orphan_paths = recovery_paths(report, RecoveryActionGroup::OrphanCleanup);
    let file_restore_packages =
        recovery_file_restore_packages(report, packages_root, RecoveryActionGroup::FileRestore);
    let mut reinstall_packages =
        recovery_package_names(report, packages_root, RecoveryActionGroup::Reinstall);
    reinstall_packages.retain(|package_name| {
        !file_restore_packages
            .iter()
            .any(|candidate| candidate.name == *package_name)
    });

    RepairPlan {
        journal_paths,
        orphan_paths,
        file_restore_packages,
        reinstall_packages,
        file_restore_count: recovery_count(report, RecoveryActionGroup::FileRestore),
        reinstall_count: recovery_count(report, RecoveryActionGroup::Reinstall),
    }
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

    journal_paths
        .iter()
        .map(|journal_path| {
            let committed = database::JournalReader::read_committed_package(journal_path)
                .with_context(|| {
                    format!(
                        "failed to parse committed journal at {}",
                        journal_path.display()
                    )
                })?;

            let command_resolution_status = classify_journal_command_resolution_status(
                committed.command_resolution.as_ref(),
                catalog_conn
                    .as_ref()
                    .and_then(|conn| current_command_resolution(conn, &committed.package.name)),
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

            Ok(JournalReplayTarget {
                journal_path: journal_path.clone(),
                committed,
                command_resolution_status,
            })
        })
        .collect()
}

fn replay_prepared_journal_targets(targets: &[JournalReplayTarget]) -> Result<usize> {
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

fn journal_commands(committed: &database::CommittedJournalPackage) -> &[String] {
    if let Some(ResolverResult::Resolved { commands, .. }) = committed.command_resolution.as_ref() {
        commands.as_slice()
    } else {
        committed.commands.as_deref().unwrap_or(&[])
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

    match resolve_command_exposure(&package, &installer) {
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

fn classify_journal_command_resolution_status(
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

fn command_resolution_is_stale(committed: &ResolverResult, current: &ResolverResult) -> bool {
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

pub fn cleanup_orphan_install_dirs(orphan_paths: &[PathBuf]) -> Result<usize> {
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

/// Resolve a catalog package for repair using the same matching policy as install.
pub fn resolve_repair_catalog_package<FChoose>(
    package_name: &str,
    choose_package: FChoose,
) -> Result<CatalogPackage>
where
    FChoose: FnMut(&str, &[CatalogPackage]) -> Result<usize>,
{
    let catalog_conn = crate::database::get_catalog_conn()?;
    resolve_repair_catalog_package_with_conn(&catalog_conn, package_name, choose_package)
}

fn resolve_repair_catalog_package_with_conn<FChoose>(
    catalog_conn: &crate::database::DbConnection,
    package_name: &str,
    choose_package: FChoose,
) -> Result<CatalogPackage>
where
    FChoose: FnMut(&str, &[CatalogPackage]) -> Result<usize>,
{
    let package_ref = PackageRef::parse(package_name)
        .with_context(|| format!("failed to parse package reference '{package_name}'"))?;

    catalog::resolve_catalog_package_ref(catalog_conn, &package_ref, choose_package)
}

/// Resolve a file-restore target and decide whether reinstall is required.
pub fn resolve_file_restore_target<FChoose>(
    package_name: &str,
    choose_package: FChoose,
) -> Result<FileRestoreResolution>
where
    FChoose: FnMut(&str, &[CatalogPackage]) -> Result<usize>,
{
    let catalog_conn = crate::database::get_catalog_conn()?;
    let conn = database::get_conn()?;
    let package =
        resolve_repair_catalog_package_with_conn(&catalog_conn, package_name, choose_package)?;
    let installed_package = database::get_package(&conn, package_name)?
        .with_context(|| format!("package '{package_name}' is not installed"))?;

    if installed_package.version != package.version.to_string() {
        return Ok(FileRestoreResolution::Reinstall(Box::new(
            FileRestoreReinstallTarget {
                catalog_package: package,
                installed_version: installed_package.version,
            },
        )));
    }

    let installers = crate::database::get_installers(&catalog_conn, &package.id)?;
    let selection_context = crate::catalog::SelectionContext::new(
        crate::windows::host_profile(),
        crate::windows::is_elevated(),
    );
    let installer = install::types::select_installer(&installers, selection_context)?;
    let engine = engines::resolve_engine_for_installer(&installer)?;

    if engine_requires_reinstall_only(engine) {
        return Ok(FileRestoreResolution::Reinstall(Box::new(
            FileRestoreReinstallTarget {
                catalog_package: package,
                installed_version: installed_package.version,
            },
        )));
    }

    Ok(FileRestoreResolution::Restore(Box::new(
        ResolvedFileRestoreTarget {
            package,
            installer,
            engine,
            installed_package,
        },
    )))
}

fn engine_requires_reinstall_only(engine: EngineKind) -> bool {
    matches!(engine, EngineKind::Font)
}

/// Reinstall a package using the exact catalog package that was already chosen.
pub fn reinstall_package<O: InstallObserver>(
    ctx: &AppContext,
    catalog_package: &CatalogPackage,
    observer: &mut O,
) -> Result<install::InstallOutcome> {
    let conn = database::get_conn()?;

    if database::get_package(&conn, &catalog_package.name)?.is_some() {
        remove::remove(&catalog_package.name, true).with_context(|| {
            format!(
                "failed to remove package before repair: {}",
                catalog_package.name
            )
        })?;
    }

    let package_ref = PackageRef::ById(
        PackageId::parse(catalog_package.id.as_str())
            .with_context(|| format!("failed to parse catalog id '{}'", catalog_package.id))?,
    );

    install::run(ctx, package_ref, false, observer)
        .with_context(|| format!("failed to reinstall package '{}'", catalog_package.name))
}

/// Restore the drifting files from a staged package tree.
pub fn restore_file_restore_target(
    target: &ResolvedFileRestoreTarget,
    target_paths: &[PathBuf],
) -> Result<usize> {
    let temp_root =
        temp_workspace::build_temp_root(&target.package.name, &target.package.version.to_string());
    cleanup_path(&temp_root)?;
    fs::create_dir_all(&temp_root)?;

    let result = (|| -> Result<usize> {
        let stage_dir = temp_root.join("stage");
        let client = install::download::build_client()?;
        let download_path = temp_root.join(installer_filename(&target.installer.url));

        install::download::download_installer(
            &client,
            &target.installer,
            &download_path,
            false,
            |_| {},
            |_| {},
        )?;

        let resolved_kind =
            engines::resolve_downloaded_installer_kind(&target.installer, &download_path)?;
        let mut resolved_installer = target.installer.clone();
        resolved_installer.kind = resolved_kind;
        let engine = engines::resolve_engine_for_installer(&resolved_installer)?;

        let _ = install::flow::execute_engine_install(
            engine,
            &resolved_installer,
            &download_path,
            &stage_dir,
            &target.package.name,
        )?;

        restore_target_files(
            &stage_dir,
            Path::new(&target.installed_package.install_dir),
            target_paths,
        )
    })();

    let _ = cleanup_path(&temp_root);

    result
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

#[cfg(test)]
mod tests {
    use super::{
        build_repair_plan, classify_journal_command_resolution_status, command_resolution_is_stale,
        engine_requires_reinstall_only, restore_target_files,
    };
    use crate::models::domains::command_resolution::{
        CommandSource, Confidence, ResolverResult, VersionScope,
    };
    use crate::models::domains::reporting::{
        DiagnosisSeverity, HealthReport, RecoveryActionGroup, RecoveryFinding, RecoveryIssueKind,
    };
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn build_repair_plan_groups_targets_and_counts_findings() {
        let report = HealthReport {
            database_path: "db.sqlite".to_string(),
            database_exists: true,
            catalog_database_path: "catalog.sqlite".to_string(),
            catalog_database_exists: true,
            install_root_source: "config".to_string(),
            install_root: "C:/Tools".to_string(),
            install_root_exists: true,
            packages_dir: "C:/Tools/packages".to_string(),
            diagnostics: Vec::new(),
            recovery_findings: vec![
                RecoveryFinding {
                    error_code: "missing_install_directory".to_string(),
                    issue_kind: RecoveryIssueKind::DiskDrift,
                    action_group: Some(RecoveryActionGroup::Reinstall),
                    description: "pkg reinstall".to_string(),
                    severity: DiagnosisSeverity::Error,
                    target_path: Some("C:/Tools/packages/Contoso.App".to_string()),
                },
                RecoveryFinding {
                    error_code: "missing_msi_file".to_string(),
                    issue_kind: RecoveryIssueKind::DiskDrift,
                    action_group: Some(RecoveryActionGroup::FileRestore),
                    description: "pkg file".to_string(),
                    severity: DiagnosisSeverity::Error,
                    target_path: Some("C:/Tools/packages/Contoso.App/bin/tool.exe".to_string()),
                },
            ],
            scan_duration: std::time::Duration::from_millis(1),
            error_count: 2,
        };

        let plan = build_repair_plan(&report, Path::new("C:/Tools/packages"));

        assert!(plan.reinstall_packages.is_empty());
        assert_eq!(plan.file_restore_packages.len(), 1);
        assert_eq!(plan.file_restore_count, 1);
        assert_eq!(plan.reinstall_count, 1);
    }

    #[test]
    fn restore_target_files_copies_staged_content() -> anyhow::Result<()> {
        let root = tempdir().expect("temp dir");
        let stage_dir = root.path().join("stage");
        let install_dir = root.path().join("packages").join("Contoso.App");
        let target_path = install_dir.join("bin").join("tool.exe");
        let staged_path = stage_dir.join("bin").join("tool.exe");

        std::fs::create_dir_all(staged_path.parent().expect("stage parent")).expect("stage dir");
        std::fs::create_dir_all(target_path.parent().expect("target parent")).expect("target dir");
        std::fs::write(&staged_path, b"restored-binary").expect("write staged file");

        let restored =
            restore_target_files(&stage_dir, &install_dir, std::slice::from_ref(&target_path))?;

        assert_eq!(restored, 1);
        assert_eq!(
            std::fs::read(&target_path).expect("read target"),
            b"restored-binary"
        );

        Ok(())
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
    fn classify_journal_command_resolution_status_tracks_fresh_and_unknown_states() {
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

        assert!(matches!(
            classify_journal_command_resolution_status(Some(&committed), Some(current)),
            super::JournalCommandResolutionStatus::Fresh
        ));
        assert!(matches!(
            classify_journal_command_resolution_status(None, None),
            super::JournalCommandResolutionStatus::Unknown
        ));
    }

    #[test]
    fn font_requires_reinstall_only() {
        assert!(engine_requires_reinstall_only(
            crate::engines::EngineKind::Font
        ));
        assert!(!engine_requires_reinstall_only(
            crate::engines::EngineKind::NativeExe
        ));
    }
}
