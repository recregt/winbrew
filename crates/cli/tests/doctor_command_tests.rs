//! Direct tests for the doctor command and its reporting helpers.
//!
//! These tests cover the wrapper behavior around health reporting, plus the
//! rendering and exit-code helpers that the command depends on.

mod common;

use std::cell::OnceCell;
use std::io::{Result as IoResult, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;
use tempfile::TempDir;
use winbrew_app::doctor::health_report;
use winbrew_cli::CommandContext;
use winbrew_cli::commands::doctor as doctor_command;
use winbrew_cli::commands::doctor::{exit_error, format_duration, render_results, write_json};
use winbrew_cli::commands::error::CommandError;
use winbrew_cli::database::{self};
use winbrew_cli::models::domains::install::InstallerType;
use winbrew_cli::models::domains::reporting::{
    DiagnosisResult, DiagnosisSeverity, HealthReport, RecoveryFinding,
};
use winbrew_cli::models::reporting::HealthScanTimings;
use winbrew_ui::{UiBuilder, UiSettings};

struct DoctorFixture {
    root: TempDir,
    db_path: PathBuf,
    db_conn: OnceCell<Connection>,
    ctx: CommandContext,
}

impl DoctorFixture {
    fn new() -> Self {
        let root = common::test_root();
        let config = common::init_database(root.path()).expect("database should initialize");
        std::fs::create_dir_all(root.path().join("packages")).expect("packages dir should exist");

        let resolved_paths = config.resolved_paths();
        let ctx = CommandContext::from_config(&config).expect("context should build");

        Self {
            root,
            db_path: resolved_paths.db,
            db_conn: OnceCell::new(),
            ctx,
        }
    }

    fn package_install_dir(&self, name: &str) -> PathBuf {
        self.root.path().join("packages").join(name)
    }

    fn conn(&self) -> &Connection {
        self.db_conn.get_or_init(|| {
            Connection::open(&self.db_path).expect("database connection should open")
        })
    }

    fn insert_installed_package(&self, name: &str, install_dir: &Path) {
        let conn = self.conn();
        let package = common::InstalledPackageBuilder::new(name)
            .version("1.0.0")
            .kind(InstallerType::Portable)
            .build(install_dir);

        database::insert_package(conn, &package).expect("package should insert");
    }
}

/// Direct doctor command coverage for the high-level wrapper behavior.
///
/// This keeps the helper tests focused while still covering the command entry
/// point for both plain text and JSON output modes.
#[test]
fn doctor_run_succeeds_in_plain_and_json_modes() {
    let fixture = DoctorFixture::new();

    doctor_command::run(&fixture.ctx, false, false).expect("plain doctor should succeed");
    doctor_command::run(&fixture.ctx, true, false).expect("json doctor should succeed");
}

#[test]
fn doctor_run_warn_as_error_returns_reported_error() {
    let fixture = DoctorFixture::new();
    let orphan_dir = fixture.package_install_dir("Contoso.Orphan");
    std::fs::create_dir_all(&orphan_dir).expect("orphan dir should exist");

    let err = doctor_command::run(&fixture.ctx, false, true)
        .expect_err("warnings should become errors in warn-as-error mode");

    let cmd_err = err.downcast_ref::<CommandError>().expect("command error");
    assert_eq!(cmd_err.exit_code(), 1);
}

#[test]
fn doctor_json_reports_corrupted_records_and_journals() {
    let fixture = DoctorFixture::new();

    let missing_install_dir = fixture.package_install_dir("Contoso.MissingInstall");
    fixture.insert_installed_package("Contoso.MissingInstall", &missing_install_dir);

    let orphan_dir = fixture.package_install_dir("Contoso.Orphan");
    std::fs::create_dir_all(&orphan_dir).expect("orphan dir should exist");

    let stale_install_dir = fixture.package_install_dir("Contoso.StaleJournal");
    std::fs::create_dir_all(&stale_install_dir).expect("stale install dir should exist");
    let stale_package = common::InstalledPackageBuilder::new("Contoso.StaleJournal")
        .version("2.0.0")
        .kind(InstallerType::Portable)
        .build(&stale_install_dir);
    database::insert_package(fixture.conn(), &stale_package).expect("stale package should insert");

    let stale_journal_path = write_committed_journal(
        fixture.root.path(),
        "Contoso.StaleJournal",
        "1.0.0",
        &stale_install_dir,
    );

    let legacy_journal_path =
        write_commit_only_journal(fixture.root.path(), "Contoso.LegacyJournal", "1.0.0");

    let output = common::run_winbrew(fixture.root.path(), &["doctor", "--json"]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "doctor should fail when broken records are present"
    );

    let report: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("doctor JSON should parse from stdout");

    assert_eq!(report["error_count"], 2);
    assert!(report["scan_duration_micros"].as_u64().is_some());

    let scan_timings = report["scan_timings"]
        .as_object()
        .expect("scan_timings should be an object");
    for key in [
        "database_connection_micros",
        "installed_packages_micros",
        "package_scan_micros",
        "msi_scan_micros",
        "orphan_scan_micros",
        "journal_scan_micros",
    ] {
        assert!(
            scan_timings
                .get(key)
                .and_then(|value| value.as_u64())
                .is_some(),
            "expected {key} to be a numeric microsecond field"
        );
    }

    let diagnostics = report["diagnostics"]
        .as_array()
        .expect("diagnostics should be an array");
    assert_eq!(diagnostics.len(), 4);
    assert_eq!(
        diagnostics
            .iter()
            .filter(|diagnostic| diagnostic["severity"] == "error")
            .count(),
        2
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["error_code"] == "missing_install_directory")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["error_code"] == "orphan_install_directory")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["error_code"] == "stale_package_journal")
    );
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic["error_code"] == "missing_journal_metadata")
    );

    let findings = report["recovery_findings"]
        .as_array()
        .expect("recovery_findings should be an array");
    assert_eq!(findings.len(), 4);

    let missing_install_finding = recovery_finding_by_code(findings, "missing_install_directory");
    assert_eq!(missing_install_finding["issue_kind"], "disk_drift");
    assert_eq!(missing_install_finding["action_group"], "reinstall");
    assert_eq!(
        missing_install_finding["target_path"],
        missing_install_dir.to_string_lossy().as_ref()
    );

    let orphan_finding = recovery_finding_by_code(findings, "orphan_install_directory");
    assert_eq!(orphan_finding["issue_kind"], "incomplete_install");
    assert_eq!(orphan_finding["action_group"], "orphan_cleanup");
    assert_eq!(
        orphan_finding["target_path"],
        orphan_dir.to_string_lossy().as_ref()
    );

    let stale_finding = recovery_finding_by_code(findings, "stale_package_journal");
    assert_eq!(stale_finding["issue_kind"], "conflict");
    assert_eq!(stale_finding["action_group"], "journal_replay");
    assert_eq!(
        stale_finding["target_path"],
        stale_journal_path.to_string_lossy().as_ref()
    );

    let legacy_finding = recovery_finding_by_code(findings, "missing_journal_metadata");
    assert_eq!(legacy_finding["issue_kind"], "recovery_trail_missing");
    assert!(legacy_finding.get("action_group").is_none());
    assert_eq!(
        legacy_finding["target_path"],
        legacy_journal_path.to_string_lossy().as_ref()
    );
}

struct SharedBuffer {
    bytes: Arc<Mutex<Vec<u8>>>,
}

impl SharedBuffer {
    fn new(bytes: Arc<Mutex<Vec<u8>>>) -> Self {
        Self { bytes }
    }
}

impl Write for SharedBuffer {
    fn write(&mut self, buffer: &[u8]) -> IoResult<usize> {
        let mut bytes = self.bytes.lock().expect("buffer lock should be available");
        bytes.extend_from_slice(buffer);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> IoResult<()> {
        Ok(())
    }
}

fn diagnosis(error_code: &str, description: &str, severity: DiagnosisSeverity) -> DiagnosisResult {
    DiagnosisResult {
        error_code: error_code.to_string(),
        description: description.to_string(),
        severity,
    }
}

fn sample_report(diagnostics: Vec<DiagnosisResult>) -> HealthReport {
    let error_count = diagnostics
        .iter()
        .filter(|diagnosis| diagnosis.severity == DiagnosisSeverity::Error)
        .count();

    HealthReport {
        database_path: "C:\\Users\\Test\\AppData\\Local\\winbrew\\data\\winbrew.db".to_string(),
        database_exists: true,
        catalog_database_path: "C:\\Users\\Test\\AppData\\Local\\winbrew\\data\\catalog.db"
            .to_string(),
        catalog_database_exists: true,
        install_root_source: "config:paths.root".to_string(),
        install_root: "C:\\Users\\Test\\AppData\\Local\\winbrew".to_string(),
        install_root_exists: true,
        packages_dir: "C:\\Users\\Test\\AppData\\Local\\winbrew\\packages".to_string(),
        diagnostics,
        recovery_findings: Vec::<RecoveryFinding>::new(),
        scan_timings: HealthScanTimings {
            database_connection: Duration::from_micros(11),
            installed_packages: Duration::from_micros(12),
            package_scan: Duration::from_micros(13),
            msi_scan: Duration::from_micros(14),
            orphan_scan: Duration::from_micros(15),
            journal_scan: Duration::from_micros(16),
        },
        scan_duration: Duration::from_micros(1_234),
        error_count,
    }
}

fn write_committed_journal(
    root: &Path,
    package_name: &str,
    version: &str,
    install_dir: &Path,
) -> PathBuf {
    let mut writer = database::JournalWriter::open_for_package(root, package_name, version)
        .expect("open committed journal");
    writer
        .append(&database::JournalEntry::Metadata {
            package_id: package_name.to_string(),
            version: version.to_string(),
            engine: "portable".to_string(),
            deployment_kind: InstallerType::Portable.deployment_kind(),
            install_dir: install_dir.to_string_lossy().into_owned(),
            dependencies: Vec::new(),
            commands: None,
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

    writer.path().to_path_buf()
}

fn write_commit_only_journal(root: &Path, package_name: &str, version: &str) -> PathBuf {
    let mut writer = database::JournalWriter::open_for_package(root, package_name, version)
        .expect("open legacy journal");
    writer
        .append(&database::JournalEntry::Commit {
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        })
        .expect("write commit");
    writer.flush().expect("flush journal");

    writer.path().to_path_buf()
}

fn recovery_finding_by_code<'a>(
    findings: &'a [serde_json::Value],
    error_code: &str,
) -> &'a serde_json::Value {
    findings
        .iter()
        .find(|finding| finding["error_code"] == error_code)
        .unwrap_or_else(|| panic!("missing recovery finding for {error_code}"))
}

#[test]
fn render_results_groups_errors_before_warnings() {
    let shared_bytes = Arc::new(Mutex::new(Vec::new()));
    let writer = SharedBuffer::new(Arc::clone(&shared_bytes));
    let err_writer = SharedBuffer::new(Arc::clone(&shared_bytes));
    let mut ui = UiBuilder::with_writer(
        writer,
        UiSettings {
            color_enabled: false,
            default_yes: false,
        },
    )
    .with_error_writer(Box::new(err_writer))
    .build();

    let warning = diagnosis(
        "orphan_install_directory",
        "orphaned package",
        DiagnosisSeverity::Warning,
    );
    let error = diagnosis(
        "missing_install_directory",
        "missing directory",
        DiagnosisSeverity::Error,
    );
    let errors = vec![&error];
    let warnings = vec![&warning];

    render_results(&mut ui, &errors, &warnings);

    let output = String::from_utf8(
        shared_bytes
            .lock()
            .expect("buffer lock should be available")
            .clone(),
    )
    .expect("rendered output should be valid UTF-8");

    let error_position = output
        .find("✘ Errors:")
        .expect("errors heading should exist");
    let warning_position = output
        .find("⚠ Warnings:")
        .expect("warnings heading should exist");

    assert!(error_position < warning_position);
    assert!(output.contains("✘   - [missing_install_directory] missing directory"));
    assert!(output.contains("⚠   - [orphan_install_directory] orphaned package"));
}

#[test]
fn render_results_handles_empty_diagnostics() {
    let shared_bytes = Arc::new(Mutex::new(Vec::new()));
    let writer = SharedBuffer::new(Arc::clone(&shared_bytes));
    let err_writer = SharedBuffer::new(Arc::clone(&shared_bytes));
    let mut ui = UiBuilder::with_writer(
        writer,
        UiSettings {
            color_enabled: false,
            default_yes: false,
        },
    )
    .with_error_writer(Box::new(err_writer))
    .build();

    let errors: Vec<&DiagnosisResult> = Vec::new();
    let warnings: Vec<&DiagnosisResult> = Vec::new();

    render_results(&mut ui, &errors, &warnings);

    let output = String::from_utf8(
        shared_bytes
            .lock()
            .expect("buffer lock should be available")
            .clone(),
    )
    .expect("rendered output should be valid UTF-8");

    assert!(output.contains("Your Winbrew installation is healthy!"));
    assert!(!output.contains("Errors:"));
    assert!(!output.contains("Warnings:"));
}

#[test]
fn format_duration_uses_seconds_milliseconds_and_microseconds() {
    assert_eq!(format_duration(Duration::from_secs(2)), "2.00s");
    assert_eq!(format_duration(Duration::from_millis(15)), "15ms");
    assert_eq!(format_duration(Duration::from_micros(7)), "7µs");
    assert_eq!(format_duration(Duration::from_micros(0)), "0µs");
}

#[test]
fn write_json_serializes_health_report() {
    let report = sample_report(vec![diagnosis(
        "missing_install_directory",
        "missing directory",
        DiagnosisSeverity::Error,
    )]);

    let mut output = Vec::new();
    write_json(&mut output, &report).expect("json should serialize");

    let value: serde_json::Value = serde_json::from_slice(&output).expect("json should parse");

    assert_eq!(value["scan_duration_micros"], 1234);
    assert_eq!(value["scan_timings"]["database_connection_micros"], 11);
    assert_eq!(value["scan_timings"]["journal_scan_micros"], 16);
    assert_eq!(value["diagnostics"][0]["severity"], "error");
}

#[test]
fn write_json_serializes_empty_diagnostics() {
    let report = sample_report(Vec::new());

    let mut output = Vec::new();
    write_json(&mut output, &report).expect("json should serialize");

    let value: serde_json::Value = serde_json::from_slice(&output).expect("json should parse");

    assert!(
        value["diagnostics"]
            .as_array()
            .is_some_and(|entries| entries.is_empty())
    );
}

#[test]
fn exit_error_uses_distinct_codes_for_errors_and_warnings() {
    let error_report = sample_report(vec![
        diagnosis(
            "missing_install_directory-1",
            "missing directory",
            DiagnosisSeverity::Error,
        ),
        diagnosis(
            "missing_install_directory-2",
            "missing directory",
            DiagnosisSeverity::Error,
        ),
        diagnosis(
            "missing_install_directory-3",
            "missing directory",
            DiagnosisSeverity::Error,
        ),
    ]);

    let warning_report = sample_report(vec![diagnosis(
        "orphan_install_directory",
        "orphaned package",
        DiagnosisSeverity::Warning,
    )]);

    let error = exit_error(error_report.error_count, 0, false).expect("error exit expected");
    let error = error
        .downcast_ref::<CommandError>()
        .expect("command error should be reported");
    assert_eq!(error.exit_code(), 2);

    let warning = exit_error(warning_report.error_count, 1, true).expect("warning exit expected");
    let warning = warning
        .downcast_ref::<CommandError>()
        .expect("command error should be reported");
    assert_eq!(warning.exit_code(), 1);
}

#[test]
fn doctor_reports_healthy_installation() {
    let fixture = DoctorFixture::new();
    let report = health_report(fixture.ctx.app()).expect("health report should build");

    assert_eq!(report.error_count, 0);
    assert!(report.diagnostics.is_empty());
}

#[test]
fn doctor_reports_missing_install_dirs_as_errors() {
    let fixture = DoctorFixture::new();
    let install_dir = fixture.package_install_dir("Contoso.Missing");
    fixture.insert_installed_package("Contoso.Missing", &install_dir);

    let report = health_report(fixture.ctx.app()).expect("health report should build");

    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnosis| diagnosis.error_code == "missing_install_directory")
    );
}

#[test]
fn doctor_reports_orphan_directories_as_warnings() {
    let fixture = DoctorFixture::new();
    let orphan_dir = fixture.package_install_dir("Contoso.Orphan");
    std::fs::create_dir_all(&orphan_dir).expect("orphan dir should exist");

    let report = health_report(fixture.ctx.app()).expect("health report should build");

    assert!(
        report
            .diagnostics
            .iter()
            .any(|diagnosis| diagnosis.error_code == "orphan_install_directory")
    );
}
