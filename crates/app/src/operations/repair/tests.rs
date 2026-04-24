use super::{
    JournalCommandResolutionStatus, JournalReplayTarget, build_repair_plan,
    classify_journal_command_resolution_status, command_resolution_is_stale,
    engine_requires_reinstall_only, restore_target_files, summarize_journal_replay_targets,
};
use crate::models::domains::command_resolution::{
    CommandSource, Confidence, ResolverResult, VersionScope,
};
use crate::models::domains::install::{EngineKind, InstallerType};
use crate::models::domains::installed::{InstalledPackage, PackageStatus};
use crate::models::domains::reporting::{
    DiagnosisSeverity, HealthReport, RecoveryActionGroup, RecoveryFinding, RecoveryIssueKind,
};
use crate::models::domains::shared::DeploymentKind;
use std::path::{Path, PathBuf};
use tempfile::tempdir;

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
fn prepare_journal_replay_targets_rejects_missing_command_resolution_metadata() -> anyhow::Result<()>
{
    let root = tempdir().expect("temp dir");
    let mut writer =
        crate::database::JournalWriter::open_for_package(root.path(), "Contoso.Legacy", "1.0.0")
            .expect("open journal");
    writer
        .append(&crate::database::JournalEntry::Metadata {
            package_id: "Contoso.Legacy".to_string(),
            version: "1.0.0".to_string(),
            engine: "portable".to_string(),
            deployment_kind: DeploymentKind::Portable,
            install_dir: root
                .path()
                .join("packages")
                .join("Contoso.Legacy")
                .to_string_lossy()
                .to_string(),
            dependencies: Vec::new(),
            commands: Some(vec!["contoso".to_string()]),
            bin: None,
            command_resolution: None,
            engine_metadata: None,
        })
        .expect("write metadata");
    writer
        .append(&crate::database::JournalEntry::Commit {
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        })
        .expect("write commit");
    writer.flush().expect("flush journal");

    let journal_path = writer.path().to_path_buf();
    let err = super::prepare_journal_replay_targets(&[journal_path])
        .expect_err("legacy journal should be rejected");

    assert!(
        err.to_string()
            .contains("missing command resolution metadata")
    );

    Ok(())
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
