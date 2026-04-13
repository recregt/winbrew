mod common;

use anyhow::Result;
use mockito::Server;
use rusqlite::{Connection, params};
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;
use std::path::PathBuf;
use tempfile::TempDir;
use winbrew_cli::database;
use winbrew_cli::models::domains::install::InstallerType;
use winbrew_cli::models::domains::installed::PackageStatus;
use winbrew_cli::models::shared::hash::HashAlgorithm;
use winbrew_core::hash::Hasher;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

struct BinaryFixture {
    root: TempDir,
    db_path: PathBuf,
    catalog_db_path: PathBuf,
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
        }
    }

    fn path(&self) -> &Path {
        self.root.path()
    }

    fn conn(&self) -> Connection {
        Connection::open(&self.db_path).expect("database connection should open")
    }

    fn catalog_conn(&self) -> Connection {
        Connection::open(&self.catalog_db_path).expect("catalog database should open")
    }

    fn run(&self, args: &[&str]) -> std::process::Output {
        common::run_winbrew(self.path(), args)
    }

    fn insert_package(&self, name: &str, version: &str, kind: InstallerType) -> std::path::PathBuf {
        let install_dir = self.path().join("packages").join(name);
        fs::create_dir_all(&install_dir).expect("install dir should exist");
        fs::write(install_dir.join("tool.exe"), b"payload").expect("install file should exist");

        let conn = self.conn();
        let package = common::InstalledPackageBuilder::new(name)
            .version(version)
            .kind(kind)
            .status(PackageStatus::Ok)
            .build(&install_dir);

        database::insert_package(&conn, &package).expect("package should insert");
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

        let conn = self.catalog_conn();
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
                Some("Synthetic package for binary install testing"),
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

#[test]
fn install_runs_through_the_binary() -> Result<()> {
    let fixture = BinaryFixture::new();
    let package_name = "Winbrew Test Zip";
    let zip_bytes = create_dummy_zip_bytes()?;
    let sha512_hash = sha512_hex(&zip_bytes);
    let mut server = Server::new();
    let installer_url = format!("{}/test.zip", server.url());
    let download_mock = server
        .mock("GET", "/test.zip")
        .with_status(200)
        .with_body(zip_bytes)
        .expect(1)
        .create();

    fixture.create_catalog_db_with_hash(package_name, &installer_url, &sha512_hash)?;

    let output = fixture.run(&["install", package_name]);

    common::assert_success(&output, "install command");
    common::assert_output_contains(&output, "Installed Winbrew Test Zip 1.0.0 into");
    download_mock.assert();

    let conn = fixture.conn();
    let stored =
        database::get_package(&conn, package_name)?.expect("package should be marked as installed");
    assert_eq!(stored.status, PackageStatus::Ok);
    assert_eq!(stored.kind, InstallerType::Zip);
    assert!(
        fixture
            .path()
            .join("packages")
            .join(package_name)
            .join("bin")
            .join("tool.exe")
            .exists()
    );

    Ok(())
}

#[test]
fn remove_runs_through_the_binary() {
    let fixture = BinaryFixture::new();
    let package_name = "Contoso.App";
    let install_dir = fixture.insert_package(package_name, "1.0.0", InstallerType::Portable);

    let output = fixture.run(&["remove", package_name, "--yes"]);

    common::assert_success(&output, "remove command");
    common::assert_output_contains(&output, "Successfully removed Contoso.App.");
    assert!(!install_dir.exists());

    let conn = fixture.conn();
    assert!(
        database::get_package(&conn, package_name)
            .expect("query should succeed")
            .is_none()
    );
}
