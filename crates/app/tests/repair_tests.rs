use anyhow::Result;
use tempfile::tempdir;

use winbrew_app::database;
use winbrew_app::repair::prepare_journal_replay_targets;
use winbrew_models::domains::shared::DeploymentKind;

#[test]
fn prepare_journal_replay_targets_rejects_missing_command_resolution_metadata() -> Result<()> {
    let root = tempdir().expect("temp dir");
    let mut writer =
        database::JournalWriter::open_for_package(root.path(), "Contoso.Legacy", "1.0.0")
            .expect("open journal");
    writer
        .append(&database::JournalEntry::Metadata {
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
        .append(&database::JournalEntry::Commit {
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        })
        .expect("write commit");
    writer.flush().expect("flush journal");

    let journal_path = writer.path().to_path_buf();
    let err = prepare_journal_replay_targets(&[journal_path])
        .expect_err("legacy journal should be rejected");

    assert!(
        err.to_string()
            .contains("missing command resolution metadata")
    );

    Ok(())
}
