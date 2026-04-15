mod common;

use anyhow::Result;
use mockito::Server;
use rusqlite::Connection;
use std::cell::OnceCell;
use std::fs;
use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use winbrew_cli::database;
use winbrew_cli::models::domains::install::{EngineMetadata, InstallerType};
use winbrew_cli::models::domains::installed::PackageStatus;
use winbrew_cli::models::shared::hash::HashAlgorithm;
use winbrew_core::hash::Hasher;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

/// RAII wrapper around mockito::Server for binary flow tests.
struct MockServer {
    server: mockito::ServerGuard,
    url: String,
}

impl MockServer {
    fn new() -> Self {
        let server = Server::new();
        let url = server.url().to_string();
        Self { server, url }
    }

    fn url(&self) -> &str {
        &self.url
    }

    fn mock_get(&mut self, path: &str, body: impl AsRef<[u8]>) -> mockito::Mock {
        self.server
            .mock("GET", path)
            .with_status(200)
            .with_body(body)
            .expect(1)
            .create()
    }
}

impl std::ops::Deref for MockServer {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.url
    }
}

struct BinaryFixture {
    root: TempDir,
    db_path: PathBuf,
    catalog_db_path: PathBuf,
    db_conn: OnceCell<Connection>,
    catalog_db_conn: OnceCell<Connection>,
}

impl BinaryFixture {
    fn new() -> Self {
        let root = common::test_root();
        let config = common::init_database(root.path()).expect("database should initialize");
        fs::create_dir_all(root.path().join("packages")).expect("packages dir should exist");

        let resolved_paths = config.resolved_paths();

        Self {
            root,
            db_path: resolved_paths.db,
            catalog_db_path: resolved_paths.catalog_db,
            db_conn: OnceCell::new(),
            catalog_db_conn: OnceCell::new(),
        }
    }

    fn path(&self) -> &Path {
        self.root.path()
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

    fn run(&self, args: &[&str]) -> std::process::Output {
        common::run_winbrew(self.path(), args)
    }

    fn package_dir(&self, name: &str) -> PathBuf {
        self.path().join("packages").join(name)
    }

    fn package_file(&self, package_name: &str, relative_path: &str) -> PathBuf {
        self.package_dir(package_name).join(relative_path)
    }

    fn assert_file_exists(&self, package_name: &str, relative_path: &str) -> Result<()> {
        let full_path = self.package_file(package_name, relative_path);
        anyhow::ensure!(
            full_path.exists(),
            "Expected file to exist: {}",
            full_path.display()
        );
        Ok(())
    }

    fn assert_dir_missing(&self, package_name: &str) -> Result<()> {
        let dir = self.package_dir(package_name);
        anyhow::ensure!(
            !dir.exists(),
            "Directory should not exist: {}",
            dir.display()
        );
        Ok(())
    }

    fn insert_package(&self, name: &str, version: &str, kind: InstallerType) -> PathBuf {
        let install_dir = self.package_dir(name);
        fs::create_dir_all(&install_dir).expect("install dir should exist");
        fs::write(install_dir.join("tool.exe"), b"payload").expect("install file should exist");

        let conn = self.conn();
        let package = common::InstalledPackageBuilder::new(name)
            .version(version)
            .kind(kind)
            .status(PackageStatus::Ok)
            .build(&install_dir);

        database::insert_package(conn, &package).expect("package should insert");
        install_dir
    }

    fn insert_native_exe_package(&self, name: &str, uninstall_command: String) -> PathBuf {
        let install_dir = self.package_dir(name);
        fs::create_dir_all(&install_dir).expect("install dir should exist");
        fs::write(install_dir.join("tool.exe"), b"payload").expect("install file should exist");

        let conn = self.conn();
        let package = common::InstalledPackageBuilder::new(name)
            .version("1.0.0")
            .kind(InstallerType::Exe)
            .engine_metadata(Some(EngineMetadata::native_exe(
                Some(uninstall_command),
                None,
            )))
            .status(PackageStatus::Ok)
            .build(&install_dir);

        database::insert_package(conn, &package).expect("package should insert");
        install_dir
    }

    fn create_catalog_db_with_hash(
        &self,
        package_name: &str,
        installer_url: &str,
        hash: &str,
    ) -> Result<()> {
        if let Some(parent) = self.catalog_db_path.parent() {
            fs::create_dir_all(parent)?;
        }

        common::seed_catalog_package(
            self.catalog_conn(),
            package_name,
            "Synthetic package for binary install testing",
            installer_url,
            hash,
        )?;

        Ok(())
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

/// Integration test for the binary install flow using a mock HTTP server.
///
/// Scenario:
/// - Seed the catalog database with a ZIP package and a matching SHA-512 hash.
/// - Serve the installer bytes from a local mock server.
///
/// Expected behavior:
/// - The command exits successfully.
/// - Output reports the installed package and destination.
/// - The package database marks the package as installed.
/// - The extracted binary exists on disk.
#[test]
fn install_runs_through_the_binary() -> Result<()> {
    let fixture = BinaryFixture::new();
    let package_name = "Winbrew Test Zip";
    let zip_bytes = create_dummy_zip_bytes()?;
    let sha512_hash = sha512_hex(&zip_bytes);
    let mut server = MockServer::new();
    let installer_url = format!("{}/test.zip", server.url());
    let download_mock = server.mock_get("/test.zip", zip_bytes);

    fixture.create_catalog_db_with_hash(package_name, &installer_url, &sha512_hash)?;

    let output = fixture.run(&["install", package_name]);

    common::assert_success(&output, "install command")?;
    common::assert_output_contains(&output, "Installed Winbrew Test Zip 1.0.0 into")?;
    download_mock.assert();

    let conn = fixture.conn();
    let stored = database::get_package(conn, package_name)?
        .ok_or_else(|| anyhow::anyhow!("package should be marked as installed"))?;
    assert_eq!(stored.status, PackageStatus::Ok);
    assert_eq!(stored.kind, InstallerType::Zip);
    fixture.assert_file_exists(package_name, "bin/tool.exe")?;

    Ok(())
}

/// Integration test for the binary remove flow.
///
/// Scenario:
/// - Seed an installed portable package in the test database.
/// - Invoke the CLI through the binary with `remove --yes`.
///
/// Expected behavior:
/// - The command exits successfully.
/// - Output confirms the package was removed.
/// - The package directory is deleted.
/// - The package row is removed from the database.
#[test]
fn remove_runs_through_the_binary() -> Result<()> {
    let fixture = BinaryFixture::new();
    let package_name = "Contoso.App";
    fixture.insert_package(package_name, "1.0.0", InstallerType::Portable);

    let output = fixture.run(&["remove", package_name, "--yes"]);

    common::assert_success(&output, "remove command")?;
    common::assert_output_contains(&output, "Successfully removed Contoso.App.")?;
    fixture.assert_dir_missing(package_name)?;

    let conn = fixture.conn();
    anyhow::ensure!(
        database::get_package(conn, package_name)?.is_none(),
        "package should be completely removed from database"
    );

    Ok(())
}

/// Integration test for the binary remove flow when the installed package is a native executable.
///
/// Scenario:
/// - Seed a NativeExe package with uninstall metadata in the test database.
/// - Invoke the CLI through the binary with `remove --yes`.
///
/// Expected behavior:
/// - The command exits successfully.
/// - The recorded uninstall command runs before filesystem cleanup.
/// - The package directory is deleted.
/// - The package row is removed from the database.
#[test]
fn remove_native_exe_runs_through_the_binary() -> Result<()> {
    let fixture = BinaryFixture::new();
    let package_name = "Contoso.NativeExe";
    let uninstall_marker = fixture.path().join("nativeexe-uninstall.log");
    let uninstall_command = format!(
        r#"powershell -NoProfile -Command "Set-Content -LiteralPath '{}' -Value 'ran'""#,
        uninstall_marker.display()
    );
    let install_dir = fixture.insert_native_exe_package(package_name, uninstall_command);

    let output = fixture.run(&["remove", package_name, "--yes"]);

    common::assert_success(&output, "remove command")?;
    common::assert_output_contains(&output, "Successfully removed Contoso.NativeExe.")?;
    anyhow::ensure!(uninstall_marker.exists(), "uninstall command should run");
    fixture.assert_dir_missing(package_name)?;
    anyhow::ensure!(!install_dir.exists(), "install directory should be removed");

    let conn = fixture.conn();
    anyhow::ensure!(
        database::get_package(conn, package_name)?.is_none(),
        "package should be completely removed from database"
    );

    Ok(())
}
