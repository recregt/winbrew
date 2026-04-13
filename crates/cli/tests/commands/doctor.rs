#[path = "../common/mod.rs"]
mod common;

use std::io::{Result as IoResult, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tempfile::TempDir;
use winbrew_app::doctor::health_report;
use winbrew_cli::CommandContext;
use winbrew_cli::commands::doctor::{exit_error, format_duration, render_results, write_json};
use winbrew_cli::commands::error::CommandError;
use winbrew_cli::database::{self, Config};
use winbrew_cli::models::domains::install::{EngineKind, InstallerType};
use winbrew_cli::models::domains::installed::{InstalledPackage, PackageStatus};
use winbrew_cli::models::domains::reporting::{
    DiagnosisResult, DiagnosisSeverity, HealthReport, RecoveryFinding,
};
use winbrew_ui::{UiBuilder, UiSettings};

struct DoctorFixture {
    root: TempDir,
    ctx: CommandContext,
}

impl DoctorFixture {
    fn new() -> Self {
        let root = common::test_root();
        common::init_database(root.path()).expect("database should initialize");
        std::fs::create_dir_all(root.path().join("packages")).expect("packages dir should exist");

        let config = Config::load_at(root.path()).expect("config should load");
        let ctx = CommandContext::from_config(&config).expect("context should build");

        Self { root, ctx }
    }

    fn root_path(&self) -> &Path {
        self.root.path()
    }

    fn package_install_dir(&self, name: &str) -> PathBuf {
        self.root.path().join("packages").join(name)
    }

    fn insert_installed_package(&self, name: &str, install_dir: &Path) {
        let conn = database::get_conn().expect("database connection should open");
        let package = InstalledPackage {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            kind: InstallerType::Portable,
            engine_kind: EngineKind::Portable,
            engine_metadata: None,
            install_dir: install_dir.to_string_lossy().to_string(),
            dependencies: Vec::new(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-10T00:00:00Z".to_string(),
        };

        database::insert_package(&conn, &package).expect("package should insert");
    }
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
        scan_duration: Duration::from_millis(1_234),
        error_count,
    }
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

    assert_eq!(value["scan_duration"], 1234);
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

    let error =
        exit_error(error_report.error_count, 0, false).expect("error code should be returned");
    let cmd_error = error
        .downcast_ref::<CommandError>()
        .expect("command error should exist");
    assert_eq!(cmd_error.exit_code(), 2);

    let warning_report = sample_report(vec![diagnosis(
        "orphan_install_directory",
        "orphaned package",
        DiagnosisSeverity::Warning,
    )]);
    let warning = exit_error(warning_report.error_count, 1, true)
        .expect("warning exit code should be returned");
    let cmd_error = warning
        .downcast_ref::<CommandError>()
        .expect("command error should exist");
    assert_eq!(cmd_error.exit_code(), 1);

    assert!(exit_error(warning_report.error_count, 1, false).is_none());
    assert!(exit_error(warning_report.error_count, 0, false).is_none());
}

#[test]
fn doctor_reports_healthy_installation() {
    let fixture = DoctorFixture::new();
    let report = health_report(fixture.ctx.app()).expect("health report should build");

    assert_eq!(report.install_root_source, "config:paths.root");
    assert_eq!(
        report.install_root,
        fixture.root_path().to_string_lossy().to_string()
    );
    assert_eq!(report.error_count, 0);
    assert!(report.diagnostics.is_empty());
}

#[test]
fn doctor_reports_orphan_directories_as_warnings() {
    let fixture = DoctorFixture::new();

    std::fs::create_dir_all(fixture.package_install_dir("Contoso.Orphan"))
        .expect("orphan dir should be created");

    let report = health_report(fixture.ctx.app()).expect("health report should build");

    assert_eq!(report.error_count, 0);
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].error_code, "orphan_install_directory");
    assert_eq!(report.diagnostics[0].severity, DiagnosisSeverity::Warning);
}

#[test]
fn doctor_reports_missing_install_dirs_as_errors() {
    let fixture = DoctorFixture::new();

    fixture.insert_installed_package("Contoso.Missing", &fixture.root_path().join("missing"));

    let report = health_report(fixture.ctx.app()).expect("health report should build");

    assert_eq!(report.error_count, 1);
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(
        report.diagnostics[0].error_code,
        "missing_install_directory"
    );
    assert_eq!(report.diagnostics[0].severity, DiagnosisSeverity::Error);
}
