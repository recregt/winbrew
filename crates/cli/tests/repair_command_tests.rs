//! Repair command integration tests for journal replay, orphan cleanup, and
//! reinstall recovery paths.

mod common;

use anyhow::Result;
use mockito::Server;
use rusqlite::{Connection, params};
use std::cell::OnceCell;
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;

use tempfile::TempDir;
use winbrew_cli::CommandContext;
use winbrew_cli::commands::repair;
use winbrew_cli::database::{self};
use winbrew_cli::models::domains::command_resolution::{
    CommandSource, Confidence, ResolverResult, VersionScope,
};
use winbrew_cli::models::domains::install::{EngineKind, InstallerType};
use winbrew_cli::models::domains::installed::PackageStatus;
use winbrew_cli::models::domains::shared::DeploymentKind;
use winbrew_cli::models::domains::shared::HashAlgorithm;
use winbrew_core::Hasher;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

struct RepairFixture {
    root: TempDir,
    db_path: std::path::PathBuf,
    catalog_db_path: std::path::PathBuf,
    db_conn: OnceCell<Connection>,
    catalog_db_conn: OnceCell<Connection>,
    ctx: CommandContext,
}

impl RepairFixture {
    fn new() -> Self {
        let root = common::test_root();
        let config = common::init_database(root.path()).expect("database should initialize");
        std::fs::create_dir_all(root.path().join("packages")).expect("packages dir should exist");

        let mut config = config;
        config.core.default_yes = true;
        let resolved_paths = config.resolved_paths();
        let ctx = CommandContext::from_config(&config).expect("context should build");

        Self {
            root,
            db_path: resolved_paths.db,
            catalog_db_path: resolved_paths.catalog_db,
            db_conn: OnceCell::new(),
            catalog_db_conn: OnceCell::new(),
            ctx,
        }
    }

    fn root_path(&self) -> &Path {
        self.root.path()
    }

    fn package_dir(&self, name: &str) -> std::path::PathBuf {
        self.root_path().join("packages").join(name)
    }

    fn conn(&self) -> &Connection {
        self.db_conn.get_or_init(|| {
            Connection::open(&self.db_path).expect("database connection should open")
        })
    }

    fn catalog_conn(&self) -> &Connection {
        self.catalog_db_conn.get_or_init(|| {
            Connection::open(&self.catalog_db_path).expect("catalog database should open")
        })
    }

    fn insert_stale_package(&self, name: &str) {
        let conn = self.conn();
        let install_dir = self.package_dir(name);
        let package = common::InstalledPackageBuilder::new(name)
            .version("0.9.0")
            .kind(InstallerType::Portable)
            .status(PackageStatus::Installing)
            .installed_at("2026-04-01T00:00:00Z")
            .build(&install_dir);

        database::insert_package(conn, &package).expect("package should insert");
    }

    fn sync_package_commands(&self, package_name: &str, commands_json: &str) {
        database::sync_package_commands(self.conn(), package_name, Some(commands_json))
            .expect("package commands should sync");
    }
}

fn create_dummy_zip_bytes() -> Result<Vec<u8>> {
    let buffer = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(buffer);
    writer.start_file("bin/tool.exe", SimpleFileOptions::default())?;
    writer.write_all(b"zip-binary")?;
    let buffer = writer.finish()?;
    Ok(buffer.into_inner())
}

fn digest_hex(algorithm: HashAlgorithm, bytes: &[u8]) -> String {
    let mut hasher = Hasher::new(algorithm);
    hasher.update(bytes);
    let digest = hasher.finalize();

    digest.iter().map(|byte| format!("{:02x}", byte)).collect()
}

fn sha512_hex(bytes: &[u8]) -> String {
    digest_hex(HashAlgorithm::Sha512, bytes)
}

fn create_catalog_db_with_hash(
    fixture: &RepairFixture,
    package_name: &str,
    installer_url: &str,
    hash: &str,
) -> Result<()> {
    common::seed_catalog_package(
        fixture.catalog_conn(),
        package_name,
        "Synthetic package for isolated repair testing",
        installer_url,
        hash,
    )?;

    Ok(())
}

fn create_catalog_db_with_installer(
    fixture: &RepairFixture,
    package_name: &str,
    installer_url: &str,
    hash: &str,
    kind: InstallerType,
    installer_switches: Option<&str>,
) -> Result<()> {
    common::seed_catalog_db_with_installer(
        &fixture.catalog_db_path,
        package_name,
        "Synthetic package for isolated repair testing",
        installer_url,
        hash,
        kind,
        installer_switches,
    )?;

    Ok(())
}

#[test]
fn repair_replays_committed_journal_into_database() {
    let fixture = RepairFixture::new();
    let package_name = "winget/Contoso.App";
    let catalog_package_name = "Contoso.App";
    let journal_install_dir = fixture.root_path().join("packages").join("Contoso.App");
    fixture.insert_stale_package(package_name);

    let zip_bytes = create_dummy_zip_bytes().expect("create zip bytes");
    let sha512_hash = sha512_hex(&zip_bytes);
    let installer_url = "https://example.invalid/contoso.zip";
    create_catalog_db_with_hash(&fixture, catalog_package_name, installer_url, &sha512_hash)
        .expect("seed catalog package");
    fixture
        .catalog_conn()
        .execute(
            "UPDATE catalog_packages SET commands = ?1 WHERE id = ?2",
            params![
                r#"["current"]"#,
                common::catalog_package_id(catalog_package_name)
            ],
        )
        .expect("seed current catalog commands");

    let mut writer =
        database::JournalWriter::open_for_package(fixture.root_path(), package_name, "1.0.0")
            .expect("open journal");
    writer
        .append(&database::JournalEntry::Metadata {
            package_id: package_name.to_string(),
            version: "1.0.0".to_string(),
            engine: "portable".to_string(),
            deployment_kind: DeploymentKind::Portable,
            install_dir: journal_install_dir.to_string_lossy().to_string(),
            dependencies: vec!["winget/Contoso.Dependency".to_string()],
            commands: Some(vec!["contoso".to_string()]),
            bin: Some(vec!["bin/tool.exe".to_string()]),
            command_resolution: Some(ResolverResult::Resolved {
                commands: vec!["contoso".to_string()],
                confidence: Confidence::High,
                sources: vec![CommandSource::PackageLevel],
                version_scope: VersionScope::Specific("1.0.0".to_string()),
                catalog_fingerprint: "sha256:deadbeef".to_string(),
            }),
            engine_metadata: None,
        })
        .expect("write metadata");
    writer
        .append(&database::JournalEntry::Commit {
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        })
        .expect("write commit");
    writer.flush().expect("flush journal");

    repair::run(&fixture.ctx, true).expect("repair should succeed");

    let conn = fixture.conn();
    let package = database::get_package(conn, package_name)
        .expect("read package")
        .expect("package should exist");

    assert_eq!(package.version, "1.0.0");
    assert_eq!(package.kind, InstallerType::Portable);
    assert_eq!(package.engine_kind, EngineKind::Portable);
    assert_eq!(
        package.install_dir,
        journal_install_dir.to_string_lossy().to_string()
    );
    assert_eq!(
        package.dependencies,
        vec!["winget/Contoso.Dependency".to_string()]
    );
    assert_eq!(package.status, PackageStatus::Ok);
    assert_eq!(package.installed_at, "2026-04-12T00:00:00Z");

    let shim_path = fixture.root_path().join("shims").join("contoso.cmd");
    assert!(shim_path.exists());
    let shim_contents = fs::read_to_string(&shim_path).expect("read shim");
    assert!(shim_contents.contains("WINBREW_SHIM_TARGET=bin\\tool.exe"));
    assert!(!shim_contents.contains("WINBREW_SHIM_NAME"));
}

#[test]
fn repair_replays_committed_journal_using_resolver_commands() {
    let fixture = RepairFixture::new();
    let package_name = "winget/Contoso.Resolved";
    let catalog_package_name = "Contoso.Resolved";
    let journal_install_dir = fixture
        .root_path()
        .join("packages")
        .join("Contoso.Resolved");
    fixture.insert_stale_package(package_name);

    let zip_bytes = create_dummy_zip_bytes().expect("create zip bytes");
    let sha512_hash = sha512_hex(&zip_bytes);
    let installer_url = "https://example.invalid/resolved.zip";
    create_catalog_db_with_hash(&fixture, catalog_package_name, installer_url, &sha512_hash)
        .expect("seed catalog package");

    let mut writer =
        database::JournalWriter::open_for_package(fixture.root_path(), package_name, "1.0.0")
            .expect("open journal");
    writer
        .append(&database::JournalEntry::Metadata {
            package_id: package_name.to_string(),
            version: "1.0.0".to_string(),
            engine: "portable".to_string(),
            deployment_kind: DeploymentKind::Portable,
            install_dir: journal_install_dir.to_string_lossy().to_string(),
            dependencies: vec!["winget/Contoso.Dependency".to_string()],
            commands: None,
            bin: Some(vec!["bin/tool.exe".to_string()]),
            command_resolution: Some(ResolverResult::Resolved {
                commands: vec!["contoso".to_string()],
                confidence: Confidence::High,
                sources: vec![CommandSource::PackageLevel],
                version_scope: VersionScope::Specific("1.0.0".to_string()),
                catalog_fingerprint: "sha256:deadbeef".to_string(),
            }),
            engine_metadata: None,
        })
        .expect("write metadata");
    writer
        .append(&database::JournalEntry::Commit {
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        })
        .expect("write commit");
    writer.flush().expect("flush journal");

    repair::run(&fixture.ctx, true).expect("repair should succeed");

    let shim_path = fixture.root_path().join("shims").join("contoso.cmd");
    assert!(shim_path.exists());
    let shim_contents = fs::read_to_string(&shim_path).expect("read shim");
    assert!(shim_contents.contains("WINBREW_SHIM_TARGET=bin\\tool.exe"));
    assert!(!shim_contents.contains("WINBREW_SHIM_NAME"));
}

#[test]
fn repair_replays_committed_journal_and_removes_stale_shims() {
    let fixture = RepairFixture::new();
    let package_name = "winget/Contoso.StaleShims";
    let catalog_package_name = "Contoso.StaleShims";
    let journal_install_dir = fixture
        .root_path()
        .join("packages")
        .join("Contoso.StaleShims");
    fixture.insert_stale_package(package_name);
    fixture.sync_package_commands(package_name, r#"["contoso","legacy"]"#);

    let legacy_shim = fixture.root_path().join("shims").join("legacy.cmd");
    fs::create_dir_all(legacy_shim.parent().expect("shim parent should exist"))
        .expect("create shim directory");
    fs::write(&legacy_shim, "legacy shim").expect("write stale shim");

    let zip_bytes = create_dummy_zip_bytes().expect("create zip bytes");
    let sha512_hash = sha512_hex(&zip_bytes);
    let installer_url = "https://example.invalid/stale-shims.zip";
    create_catalog_db_with_hash(&fixture, catalog_package_name, installer_url, &sha512_hash)
        .expect("seed catalog package");

    let mut writer =
        database::JournalWriter::open_for_package(fixture.root_path(), package_name, "1.0.0")
            .expect("open journal");
    writer
        .append(&database::JournalEntry::Metadata {
            package_id: package_name.to_string(),
            version: "1.0.0".to_string(),
            engine: "portable".to_string(),
            deployment_kind: DeploymentKind::Portable,
            install_dir: journal_install_dir.to_string_lossy().to_string(),
            dependencies: vec!["winget/Contoso.Dependency".to_string()],
            commands: None,
            bin: Some(vec!["bin/tool.exe".to_string()]),
            command_resolution: Some(ResolverResult::Resolved {
                commands: vec!["contoso".to_string()],
                confidence: Confidence::High,
                sources: vec![CommandSource::PackageLevel],
                version_scope: VersionScope::Specific("1.0.0".to_string()),
                catalog_fingerprint: "sha256:deadbeef".to_string(),
            }),
            engine_metadata: None,
        })
        .expect("write metadata");
    writer
        .append(&database::JournalEntry::Commit {
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        })
        .expect("write commit");
    writer.flush().expect("flush journal");

    repair::run(&fixture.ctx, true).expect("repair should succeed");

    let current_shim = fixture.root_path().join("shims").join("contoso.cmd");
    assert!(current_shim.exists());
    assert!(!legacy_shim.exists());

    let shim_contents = fs::read_to_string(&current_shim).expect("read current shim");
    assert!(shim_contents.contains("WINBREW_SHIM_TARGET=bin\\tool.exe"));
    assert!(!shim_contents.contains("WINBREW_SHIM_NAME"));
}

#[test]
fn repair_reports_journal_command_resolution_summary() -> Result<()> {
    let fixture = RepairFixture::new();
    let package_name = "winget/Contoso.Summary";
    let catalog_package_name = "Contoso.Summary";
    let journal_install_dir = fixture.root_path().join("packages").join("Contoso.Summary");
    fixture.insert_stale_package(package_name);

    let zip_bytes = create_dummy_zip_bytes()?;
    let sha512_hash = sha512_hex(&zip_bytes);
    let installer_url = "https://example.invalid/summary.zip";
    create_catalog_db_with_hash(&fixture, catalog_package_name, installer_url, &sha512_hash)?;
    fixture
        .catalog_conn()
        .execute(
            "UPDATE catalog_packages SET commands = ?1 WHERE id = ?2",
            params![
                r#"["current"]"#,
                common::catalog_package_id(catalog_package_name)
            ],
        )
        .expect("seed current catalog commands");

    let mut writer =
        database::JournalWriter::open_for_package(fixture.root_path(), package_name, "1.0.0")
            .expect("open journal");
    writer
        .append(&database::JournalEntry::Metadata {
            package_id: package_name.to_string(),
            version: "1.0.0".to_string(),
            engine: "portable".to_string(),
            deployment_kind: DeploymentKind::Portable,
            install_dir: journal_install_dir.to_string_lossy().to_string(),
            dependencies: vec!["winget/Contoso.Dependency".to_string()],
            commands: None,
            bin: Some(vec!["bin/tool.exe".to_string()]),
            command_resolution: Some(ResolverResult::Resolved {
                commands: vec!["contoso".to_string()],
                confidence: Confidence::High,
                sources: vec![CommandSource::PackageLevel],
                version_scope: VersionScope::Specific("1.0.0".to_string()),
                catalog_fingerprint: "sha256:deadbeef".to_string(),
            }),
            engine_metadata: None,
        })
        .expect("write metadata");
    writer
        .append(&database::JournalEntry::Commit {
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        })
        .expect("write commit");
    writer.flush().expect("flush journal");

    let output = common::run_winbrew(fixture.root_path(), &["repair", "-y"]);
    common::assert_success(&output, "winbrew repair")?;
    common::assert_output_contains(
        &output,
        "Journal command resolution: 0 fresh, 1 stale, 0 unknown.",
    )?;

    Ok(())
}

#[test]
fn repair_removes_orphan_install_directory() {
    let fixture = RepairFixture::new();
    let orphan_dir = fixture.package_dir("Contoso.Orphan");
    std::fs::create_dir_all(&orphan_dir).expect("orphan dir should exist");

    assert!(orphan_dir.exists());

    repair::run(&fixture.ctx, true).expect("repair should succeed");

    assert!(!orphan_dir.exists());
}

#[test]
fn repair_reinstalls_missing_package_from_catalog() -> Result<()> {
    let fixture = RepairFixture::new();
    let package_name = "Winbrew Repair Zip";
    let install_dir = fixture.package_dir(package_name);

    let zip_bytes = create_dummy_zip_bytes()?;
    let sha512_hash = sha512_hex(&zip_bytes);

    let mut server = Server::new();
    let installer_url = format!("{}/repair.zip", server.url());
    let download_mock = server
        .mock("GET", "/repair.zip")
        .with_status(200)
        .with_body(zip_bytes)
        .expect(1)
        .create();

    if let Some(parent) = fixture.catalog_db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    create_catalog_db_with_hash(&fixture, package_name, &installer_url, &sha512_hash)?;

    let conn = fixture.conn();
    let package = common::InstalledPackageBuilder::new(package_name)
        .version("0.9.0")
        .kind(InstallerType::Zip)
        .build(&install_dir);

    database::insert_package(conn, &package).expect("package should insert");

    repair::run(&fixture.ctx, true).expect("repair should succeed");

    let conn = fixture.conn();
    let package = database::get_package(conn, package_name)
        .expect("read package")
        .expect("package should exist");

    assert_eq!(package.version, "1.0.0");
    assert_eq!(package.kind, InstallerType::Zip);
    assert_eq!(package.engine_kind, EngineKind::Zip);
    assert_eq!(
        package.install_dir,
        install_dir.to_string_lossy().to_string()
    );
    assert!(install_dir.join("bin").join("tool.exe").exists());
    download_mock.assert();

    Ok(())
}

#[test]
fn repair_reinstalls_native_exe_from_catalog() -> Result<()> {
    let fixture = RepairFixture::new();
    let package_name = "Winbrew Repair NativeExe";
    let install_dir = fixture.package_dir(package_name);

    let cmd_path = std::path::PathBuf::from(
        std::env::var_os("ComSpec").expect("ComSpec should be set on Windows"),
    );
    let cmd_bytes = fs::read(&cmd_path)?;
    let sha512_hash = sha512_hex(&cmd_bytes);

    let mut server = Server::new();
    let installer_url = format!("{}/repair.exe", server.url());
    let download_mock = server
        .mock("GET", "/repair.exe")
        .with_status(200)
        .with_body(cmd_bytes)
        .expect(1)
        .create();

    if let Some(parent) = fixture.catalog_db_path.parent() {
        fs::create_dir_all(parent)?;
    }
    create_catalog_db_with_installer(
        &fixture,
        package_name,
        &installer_url,
        &sha512_hash,
        InstallerType::Exe,
        Some("/C exit 0"),
    )?;

    let conn = fixture.conn();
    let package = common::InstalledPackageBuilder::new(package_name)
        .version("0.9.0")
        .kind(InstallerType::Exe)
        .build(&install_dir);

    database::insert_package(conn, &package).expect("package should insert");

    repair::run(&fixture.ctx, true).expect("repair should succeed");

    let conn = fixture.conn();
    let package = database::get_package(conn, package_name)
        .expect("read package")
        .expect("package should exist");

    assert_eq!(package.version, "1.0.0");
    assert_eq!(package.kind, InstallerType::Exe);
    assert_eq!(package.engine_kind, EngineKind::NativeExe);
    assert_eq!(
        package.install_dir,
        install_dir.to_string_lossy().to_string()
    );
    download_mock.assert();

    Ok(())
}

#[test]
fn repair_is_a_noop_when_no_recovery_targets_exist() {
    let fixture = RepairFixture::new();

    repair::run(&fixture.ctx, true).expect("repair should succeed with no targets");
}
