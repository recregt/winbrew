use anyhow::Result;
use std::fs;
use std::path::Path;

use winbrew_app::install;
use winbrew_app::install::InstallObserver;
use winbrew_app::{AppContext, database};
use winbrew_models::domains::catalog::CatalogPackage;
use winbrew_models::domains::install::{EngineKind, InstallerType};
use winbrew_models::domains::installed::PackageStatus;
use winbrew_models::domains::package::{PackageId, PackageName, PackageRef};
use winbrew_models::shared::HashAlgorithm as CatalogHashAlgorithm;
use winbrew_testing::{
    Mock, MockServer, create_dummy_zip_bytes, init_database, md5_hex, reset_install_state,
    sha1_hex, sha512_hex, test_root,
};

struct InstallTestFixture {
    ctx: AppContext,
    package_name: String,
    server: Option<MockServer>,
    download_mock: Option<Mock>,
}

struct NoopInstallObserver;

impl InstallObserver for NoopInstallObserver {
    fn choose_package(
        &mut self,
        _query: &str,
        _matches: &[CatalogPackage],
    ) -> anyhow::Result<usize> {
        unreachable!("install should not prompt for an exact match")
    }

    fn on_start(&mut self, _total_bytes: Option<u64>) {}

    fn on_progress(&mut self, _downloaded_bytes: u64) {}
}

impl InstallTestFixture {
    fn from_catalog(root: &Path, installer_url: &str, hash: &str) -> Result<Self> {
        Self::from_catalog_with_installer(
            root,
            "Winbrew Test Zip",
            installer_url,
            hash,
            InstallerType::Zip,
            None,
        )
    }

    fn from_catalog_with_installer(
        root: &Path,
        package_name: &str,
        installer_url: &str,
        hash: &str,
        kind: InstallerType,
        installer_switches: Option<&str>,
    ) -> Result<Self> {
        let config = init_database(root)?;
        reset_install_state(root)?;

        let catalog_db_dir = root.join("data").join("db");
        fs::create_dir_all(&catalog_db_dir)?;
        winbrew_testing::seed_catalog_db_with_installer(
            &catalog_db_dir.join("catalog.db"),
            package_name,
            "Synthetic package for isolated install testing",
            installer_url,
            hash,
            kind,
            installer_switches,
        )?;

        let ctx = AppContext::from_config(&config)?;

        Ok(Self {
            ctx,
            package_name: package_name.to_string(),
            server: None,
            download_mock: None,
        })
    }

    fn from_zip(root: &Path, zip_bytes: Vec<u8>, hash: &str) -> Result<Self> {
        let mut server = MockServer::new();

        let installer_url = format!("{}/test.zip", server.url());
        let download_mock = server.mock_get("/test.zip", zip_bytes);

        let mut fixture = Self::from_catalog(root, &installer_url, hash)?;
        fixture.server = Some(server);
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
    assert_eq!(stored.status, PackageStatus::Ok);
    assert_eq!(stored.kind, InstallerType::Zip);
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
            id: "Winbrew.Test.Zip".to_string(),
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
fn install_runs_native_exe_end_to_end_in_an_isolated_root() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();

    let cmd_path = std::path::PathBuf::from(
        std::env::var_os("ComSpec").expect("ComSpec should be set on Windows"),
    );
    let cmd_bytes = fs::read(&cmd_path)?;
    let sha512_hash = sha512_hex(&cmd_bytes);

    let mut server = MockServer::new();
    let installer_url = format!("{}/setup.exe", server.url());
    let download_mock = server.mock_get("/setup.exe", cmd_bytes);

    let fixture = InstallTestFixture::from_catalog_with_installer(
        root,
        "Winbrew Test NativeExe",
        &installer_url,
        &sha512_hash,
        InstallerType::Exe,
        Some("/C exit 0"),
    )?;

    let outcome = fixture.run_install(false)?;

    let result = outcome.result;
    let install_dir = fixture.ctx.paths.packages.join(&fixture.package_name);
    assert_eq!(result.name, "Winbrew Test NativeExe");
    assert_eq!(result.version, "1.0.0");
    assert_eq!(result.install_dir, install_dir.to_string_lossy());
    assert!(install_dir.exists());

    let conn = database::get_conn()?;
    let stored = database::get_package(&conn, "Winbrew Test NativeExe")?
        .expect("package should be marked as installed");
    assert_eq!(stored.status, PackageStatus::Ok);
    assert_eq!(stored.kind, InstallerType::Exe);
    assert_eq!(stored.engine_kind, EngineKind::NativeExe);
    download_mock.assert();

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
        [CatalogHashAlgorithm::Sha1]
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

    let mut server = MockServer::new();
    let installer_url = format!("{}/test.zip", server.url());
    let download_mock = server.mock_get_with_status("/test.zip", 500, "boom");

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
    assert_eq!(stored.status, PackageStatus::Failed);

    Ok(())
}
