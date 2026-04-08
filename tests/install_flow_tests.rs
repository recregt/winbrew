#[path = "common/shared_root.rs"]
mod shared_root;

use anyhow::Result;
use mockito::{Mock, Server, ServerGuard};
use rusqlite::{Connection, params};
use shared_root::test_root;
use std::fs;
use std::io::{Cursor, Write};
use std::path::Path;
use winbrew::AppContext;
use winbrew::core::hash::{HashAlgorithm, Hasher};
use winbrew::database;
use winbrew::models::{PackageId, PackageName, PackageRef};
use winbrew::services::app::install;
use winbrew::services::app::install::InstallObserver;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

fn create_dummy_zip_bytes() -> Result<Vec<u8>> {
    let buffer = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(buffer);
    writer.start_file("bin/tool.exe", SimpleFileOptions::default())?;
    writer.write_all(b"zip-binary")?;
    let buffer = writer.finish()?;
    Ok(buffer.into_inner())
}

fn reset_install_state(root: &Path) -> Result<()> {
    let conn = database::get_conn()?;
    conn.execute("DELETE FROM installed_packages", [])?;

    let packages_dir = root.join("packages");
    if packages_dir.exists() {
        fs::remove_dir_all(&packages_dir)?;
    }
    fs::create_dir_all(&packages_dir)?;

    Ok(())
}

fn digest_hex(algorithm: HashAlgorithm, bytes: &[u8]) -> String {
    let mut hasher = Hasher::new(algorithm);
    hasher.update(bytes);
    let digest = hasher.finalize();

    digest.iter().map(|byte| format!("{:02x}", byte)).collect()
}

fn md5_hex(bytes: &[u8]) -> String {
    digest_hex(HashAlgorithm::Md5, bytes)
}

fn sha1_hex(bytes: &[u8]) -> String {
    digest_hex(HashAlgorithm::Sha1, bytes)
}

fn sha512_hex(bytes: &[u8]) -> String {
    digest_hex(HashAlgorithm::Sha512, bytes)
}

fn init_context(root: &Path) -> Result<AppContext> {
    let config = database::Config::load_at(root)?;
    let context = AppContext::from_config(config)?;
    database::init(&context.paths)?;
    Ok(context)
}

struct InstallTestFixture {
    ctx: AppContext,
    package_name: String,
    _server: Option<ServerGuard>,
    download_mock: Option<Mock>,
}

struct NoopInstallObserver;

impl InstallObserver for NoopInstallObserver {
    fn choose_package(
        &mut self,
        _query: &str,
        _matches: &[winbrew::models::CatalogPackage],
    ) -> anyhow::Result<usize> {
        unreachable!("install should not prompt for an exact match")
    }

    fn on_start(&mut self, _total_bytes: Option<u64>) {}

    fn on_progress(&mut self, _downloaded_bytes: u64) {}
}

impl InstallTestFixture {
    fn from_catalog(root: &Path, installer_url: &str, hash: &str) -> Result<Self> {
        reset_install_state(root)?;

        let catalog_db_dir = root.join("data").join("db");
        fs::create_dir_all(&catalog_db_dir)?;
        create_catalog_db_with_hash(&catalog_db_dir.join("catalog.db"), installer_url, hash)?;

        let ctx = init_context(root)?;

        Ok(Self {
            ctx,
            package_name: "Winbrew Test Zip".to_string(),
            _server: None,
            download_mock: None,
        })
    }

    fn from_zip(root: &Path, zip_bytes: Vec<u8>, hash: &str) -> Result<Self> {
        let mut server = Server::new();

        let installer_url = format!("{}/test.zip", server.url());
        let download_mock = server
            .mock("GET", "/test.zip")
            .with_status(200)
            .with_body(zip_bytes)
            .expect(1)
            .create();

        let mut fixture = Self::from_catalog(root, &installer_url, hash)?;
        fixture._server = Some(server);
        fixture.download_mock = Some(download_mock);
        Ok(fixture)
    }

    fn assert_downloaded(&self) {
        if let Some(download_mock) = &self.download_mock {
            download_mock.assert();
        }
    }

    fn run_install(&self, ignore_checksum_security: bool) -> Result<install::InstallOutcome> {
        let mut observer = NoopInstallObserver;
        Ok(install::run(
            &self.ctx,
            PackageRef::ByName(PackageName::parse(self.package_name.as_str())?),
            ignore_checksum_security,
            &mut observer,
        )?)
    }

    fn run_install_ref(
        &self,
        package_ref: PackageRef,
        ignore_checksum_security: bool,
    ) -> Result<install::InstallOutcome> {
        let mut observer = NoopInstallObserver;
        Ok(install::run(
            &self.ctx,
            package_ref,
            ignore_checksum_security,
            &mut observer,
        )?)
    }
}

#[test]
fn install_runs_end_to_end_in_an_isolated_root() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();

    let zip_bytes = create_dummy_zip_bytes()?;
    let sha512_hash = sha512_hex(&zip_bytes);
    let fixture = InstallTestFixture::from_zip(root, zip_bytes, &sha512_hash)?;

    let outcome = fixture.run_install(false)?;

    let result = outcome.result;
    let install_dir = fixture.ctx.paths.packages.join(&fixture.package_name);
    assert_eq!(result.name, "Winbrew Test Zip");
    assert_eq!(result.version, "1.0.0");
    assert_eq!(result.install_dir, install_dir.to_string_lossy());
    assert!(install_dir.join("bin").join("tool.exe").exists());

    let conn = database::get_conn()?;
    let stored = database::get_package(&conn, "Winbrew Test Zip")?
        .expect("package should be marked as installed");
    assert_eq!(stored.status, winbrew::models::PackageStatus::Ok);
    assert_eq!(stored.kind, winbrew::models::InstallerType::Zip);
    fixture.assert_downloaded();

    Ok(())
}

#[test]
fn install_supports_explicit_winget_ids() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();

    let zip_bytes = create_dummy_zip_bytes()?;
    let sha512_hash = sha512_hex(&zip_bytes);
    let fixture = InstallTestFixture::from_zip(root, zip_bytes, &sha512_hash)?;

    let outcome = fixture.run_install_ref(
        PackageRef::ById(PackageId::Winget {
            id: "Winbrew.TestZip".to_string(),
        }),
        false,
    )?;

    let result = outcome.result;
    assert_eq!(result.name, "Winbrew Test Zip");
    assert_eq!(result.version, "1.0.0");
    fixture.assert_downloaded();

    Ok(())
}

#[test]
fn install_rejects_md5_without_override() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();

    let installer_url = "https://example.invalid/test.zip".to_string();
    let md5_hash = "d41d8cd98f00b204e9800998ecf8427e".to_string();
    let fixture = InstallTestFixture::from_catalog(root, &installer_url, &md5_hash)?;

    let err = fixture
        .run_install(false)
        .expect_err("md5 should be rejected without override");

    assert!(
        err.to_string()
            .contains("MD5 checksums are disabled by default")
    );

    Ok(())
}

#[test]
fn install_rejects_sha1_without_override() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();

    let installer_url = "https://example.invalid/test.zip".to_string();
    let sha1_hash = "a9993e364706816aba3e25717850c26c9cd0d89d".to_string();
    let fixture = InstallTestFixture::from_catalog(root, &installer_url, &sha1_hash)?;

    let err = fixture
        .run_install(false)
        .expect_err("sha1 should be rejected without override");

    assert!(
        err.to_string()
            .contains("SHA1 checksums are disabled by default")
    );

    Ok(())
}

#[test]
fn install_allows_sha1_with_override() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();

    let zip_bytes = create_dummy_zip_bytes()?;
    let sha1_hash = sha1_hex(&zip_bytes);
    let fixture = InstallTestFixture::from_zip(root, zip_bytes, &sha1_hash)?;

    let outcome = fixture.run_install(true)?;

    assert!(matches!(
        outcome.legacy_checksum_algorithms.as_slice(),
        [winbrew::core::hash::HashAlgorithm::Sha1]
    ));
    fixture.assert_downloaded();
    Ok(())
}

#[test]
fn install_allows_md5_with_override() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();

    let zip_bytes = create_dummy_zip_bytes()?;
    let md5_hash = md5_hex(&zip_bytes);
    let fixture = InstallTestFixture::from_zip(root, zip_bytes, &md5_hash)?;

    let outcome = fixture.run_install(true)?;

    let result = outcome.result;
    let install_dir = fixture.ctx.paths.packages.join(&fixture.package_name);
    assert_eq!(result.name, "Winbrew Test Zip");
    assert!(install_dir.join("bin").join("tool.exe").exists());
    fixture.assert_downloaded();
    Ok(())
}

#[test]
fn install_rolls_back_on_download_failure() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();

    let mut server = Server::new();
    let installer_url = format!("{}/test.zip", server.url());
    let download_mock = server
        .mock("GET", "/test.zip")
        .with_status(500)
        .with_body("boom")
        .expect(1)
        .create();

    let fixture = InstallTestFixture::from_catalog(root, &installer_url, "")?;

    let err = fixture
        .run_install(false)
        .expect_err("download failures should bubble up");

    download_mock.assert();
    assert!(err.to_string().contains("installer request failed"));

    let install_dir = fixture.ctx.paths.packages.join(&fixture.package_name);
    assert!(!install_dir.exists());

    let conn = database::get_conn()?;
    let stored = database::get_package(&conn, &fixture.package_name)?
        .expect("package should remain tracked after rollback");
    assert_eq!(stored.status, winbrew::models::PackageStatus::Failed);

    Ok(())
}

fn create_catalog_db_with_hash(path: &Path, installer_url: &str, hash: &str) -> Result<()> {
    let conn = Connection::open(path)?;

    conn.execute_batch(include_str!("fixtures/catalog_schema.sql"))?;

    conn.execute("DELETE FROM catalog_installers", [])?;
    conn.execute("DELETE FROM catalog_packages", [])?;

    conn.execute(
        r#"
        INSERT INTO catalog_packages (
            id, name, version, description, homepage, license, publisher
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        params![
            "winget/Winbrew.TestZip",
            "Winbrew Test Zip",
            "1.0.0",
            Some("Synthetic package for isolated install testing"),
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
        params!["winget/Winbrew.TestZip", installer_url, hash, "", "zip",],
    )?;

    Ok(())
}
