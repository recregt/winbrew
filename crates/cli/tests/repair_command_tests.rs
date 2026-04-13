mod common;

use anyhow::Result;
use mockito::Server;
use rusqlite::{Connection, params};
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;

use tempfile::TempDir;
use winbrew_cli::CommandContext;
use winbrew_cli::commands::repair;
use winbrew_cli::database::{self};
use winbrew_cli::models::domains::install::{EngineKind, InstallerType};
use winbrew_cli::models::domains::installed::PackageStatus;
use winbrew_cli::models::domains::shared::HashAlgorithm;
use winbrew_core::Hasher;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

struct RepairFixture {
    root: TempDir,
    db_path: std::path::PathBuf,
    catalog_db_path: std::path::PathBuf,
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
            ctx,
        }
    }

    fn root_path(&self) -> &Path {
        self.root.path()
    }

    fn insert_stale_package(&self, name: &str) {
        let conn = Connection::open(&self.db_path).expect("database connection should open");
        let install_dir = self.root_path().join("packages").join(name);
        let package = common::InstalledPackageBuilder::new(name)
            .version("0.9.0")
            .kind(InstallerType::Portable)
            .status(PackageStatus::Installing)
            .installed_at("2026-04-01T00:00:00Z")
            .build(&install_dir);

        database::insert_package(&conn, &package).expect("package should insert");
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
    path: &Path,
    package_name: &str,
    installer_url: &str,
    hash: &str,
) -> Result<()> {
    let conn = Connection::open(path)?;

    conn.execute_batch(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/catalog_schema.sql"
    )))?;

    conn.execute("DELETE FROM catalog_installers", [])?;
    conn.execute("DELETE FROM catalog_packages", [])?;

    let package_id = common::catalog_package_id(package_name);

    conn.execute(
        r#"
        INSERT INTO catalog_packages (
            id, name, version, description, homepage, license, publisher
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        params![
            package_id.clone(),
            package_name,
            "1.0.0",
            Some("Synthetic package for isolated repair testing"),
            Option::<String>::None,
            Option::<String>::None,
            Some("Winbrew Ltd."),
        ],
    )?;

    conn.execute(
        r#"
        INSERT INTO catalog_installers (
            package_id, url, hash, arch, type
        ) VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        params![package_id, installer_url, hash, "", "zip"],
    )?;

    Ok(())
}

#[test]
fn repair_replays_committed_journal_into_database() {
    let fixture = RepairFixture::new();
    let package_name = "winget/Contoso.App";
    let journal_install_dir = fixture.root_path().join("packages").join("Contoso.App");
    fixture.insert_stale_package(package_name);

    let mut writer =
        database::JournalWriter::open_for_package(fixture.root_path(), package_name, "1.0.0")
            .expect("open journal");
    writer
        .append(&database::JournalEntry::Metadata {
            package_id: package_name.to_string(),
            version: "1.0.0".to_string(),
            engine: "portable".to_string(),
            install_dir: journal_install_dir.to_string_lossy().to_string(),
            dependencies: vec!["winget/Contoso.Dependency".to_string()],
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

    let conn = Connection::open(&fixture.db_path).expect("database connection should open");
    let package = database::get_package(&conn, package_name)
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
}

#[test]
fn repair_removes_orphan_install_directory() {
    let fixture = RepairFixture::new();
    let orphan_dir = fixture.root_path().join("packages").join("Contoso.Orphan");
    std::fs::create_dir_all(&orphan_dir).expect("orphan dir should exist");

    assert!(orphan_dir.exists());

    repair::run(&fixture.ctx, true).expect("repair should succeed");

    assert!(!orphan_dir.exists());
}

#[test]
fn repair_reinstalls_missing_package_from_catalog() -> Result<()> {
    let fixture = RepairFixture::new();
    let package_name = "Winbrew Repair Zip";
    let install_dir = fixture.root_path().join("packages").join(package_name);

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
    create_catalog_db_with_hash(
        &fixture.catalog_db_path,
        package_name,
        &installer_url,
        &sha512_hash,
    )?;

    let conn = Connection::open(&fixture.db_path).expect("database connection should open");
    let package = common::InstalledPackageBuilder::new(package_name)
        .version("0.9.0")
        .kind(InstallerType::Zip)
        .build(&install_dir);

    database::insert_package(&conn, &package).expect("package should insert");

    repair::run(&fixture.ctx, true).expect("repair should succeed");

    let conn = Connection::open(&fixture.db_path).expect("database connection should open");
    let package = database::get_package(&conn, package_name)
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
