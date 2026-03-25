#[path = "common/mod.rs"]
mod common;

use anyhow::Result;
use common::{TestEnvVar, env_lock};
use common::{fixtures, mock_server::MockServer};
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use winbrew::core::install::{InstallPlan, install_root};
use winbrew::core::paths;
use winbrew::database;
use winbrew::manifest::Source;
use winbrew::models::PackageStatus;
use winbrew::services::install::portable;

#[test]
fn download_and_verify_fetches_from_mock_server() {
    let mut server = MockServer::new();
    let body = "portable package contents";
    let checksum = hex::encode(Sha256::digest(body.as_bytes()));
    let mock = server.get_text("/portable.zip", body);

    let temp_dir = tempdir().expect("temporary directory should be created");
    let dest = temp_dir.path().join("portable.zip");
    let settings = winbrew::core::network::NetworkSettings {
        timeout_secs: 5,
        proxy_url: None,
        github_token: None,
    };
    let url = format!("{}/portable.zip", server.url());

    winbrew::core::network::download_and_verify(&settings, &url, &dest, &checksum, |_, _| {})
        .expect("download should succeed");

    mock.assert();
    assert_eq!(
        std::fs::read_to_string(&dest).expect("downloaded file should exist"),
        body
    );
    assert!(!dest.with_extension("part").exists());
}

#[test]
fn portable_install_downloads_from_mock_server_and_updates_database() -> Result<()> {
    let _guard = env_lock();
    let temp_root = tempdir()?;
    let _root_env = TestEnvVar::set("WINBREW_ROOT", temp_root.path().to_string_lossy().as_ref());

    fixtures::init_database_root(temp_root.path())?;

    let mut server = MockServer::new();
    let body = "portable package contents";
    let checksum = hex::encode(Sha256::digest(body.as_bytes()));
    let mock = server.get_text("/PortableApp.zip", body);

    let name = "Contoso.PortableApp";
    let version = "1.0.0";
    let source = Source {
        url: format!("{}/PortableApp.zip", server.url()),
        checksum,
        kind: "portable".to_string(),
    };

    let install_root = install_root();
    let install_dir = paths::package_dir_at(&install_root, name);
    let cache_file = paths::cache_file_at(&install_root, name, version, "zip");

    let context = InstallPlan {
        name: name.to_string(),
        package_version: version.to_string(),
        source,
        cache_file: cache_file.clone(),
        install_dir: install_dir.clone(),
        backup_dir: install_dir.with_extension("backup"),
        product_code: None,
        dependencies: vec!["Example.Dependency".to_string()],
    };

    let conn = database::get_conn()?;
    let mut progress_events = Vec::new();

    portable::install(&conn, &context, &mut |current, total| {
        progress_events.push((current, total));
    })?;

    mock.assert();

    let install_file = install_dir.join("PortableApp.zip");
    assert_eq!(std::fs::read_to_string(&install_file)?, body);
    assert_eq!(std::fs::read_to_string(&cache_file)?, body);

    let package = database::get_package(&conn, name)?.expect("installed package should exist");
    assert_eq!(package.status, PackageStatus::Ok);
    assert_eq!(package.version, version);
    assert_eq!(package.kind, "portable");
    assert_eq!(package.dependencies, vec!["Example.Dependency".to_string()]);
    assert!(!progress_events.is_empty());

    Ok(())
}

#[test]
fn portable_install_rejects_checksum_mismatch_and_cleans_up() -> Result<()> {
    let _guard = env_lock();
    let temp_root = tempdir()?;
    let _root_env = TestEnvVar::set("WINBREW_ROOT", temp_root.path().to_string_lossy().as_ref());

    fixtures::init_database_root(temp_root.path())?;

    let mut server = MockServer::new();
    let body = "portable package contents";
    let mock = server.get_text("/PortableApp.zip", body);

    let name = "Contoso.BadChecksumApp";
    let version = "1.0.0";
    let source = Source {
        url: format!("{}/PortableApp.zip", server.url()),
        checksum: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string(),
        kind: "portable".to_string(),
    };

    let install_root = install_root();
    let install_dir = paths::package_dir_at(&install_root, name);
    let cache_file = paths::cache_file_at(&install_root, name, version, "zip");

    let context = InstallPlan {
        name: name.to_string(),
        package_version: version.to_string(),
        source,
        cache_file: cache_file.clone(),
        install_dir: install_dir.clone(),
        backup_dir: install_dir.with_extension("backup"),
        product_code: None,
        dependencies: vec![],
    };

    let conn = database::get_conn()?;
    let mut progress_events = Vec::new();

    let error = portable::install(&conn, &context, &mut |current, total| {
        progress_events.push((current, total));
    })
    .expect_err("checksum mismatch should fail");

    mock.assert();
    assert!(
        error
            .to_string()
            .contains("download and verification failed")
    );
    assert!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("checksum mismatch"))
    );
    assert!(!cache_file.with_extension("part").exists());
    assert!(!install_dir.exists());
    assert!(!progress_events.is_empty());

    let package =
        database::get_package(&conn, name)?.expect("failed install should still be tracked");
    assert_eq!(package.status, PackageStatus::Failed);
    assert_eq!(package.version, version);
    assert_eq!(package.kind, "portable");

    Ok(())
}

#[test]
fn portable_install_propagates_server_error_and_cleans_up() -> Result<()> {
    let _guard = env_lock();
    let temp_root = tempdir()?;
    let _root_env = TestEnvVar::set("WINBREW_ROOT", temp_root.path().to_string_lossy().as_ref());

    fixtures::init_database_root(temp_root.path())?;

    let mut server = MockServer::new();
    let error_mock = server.get_text_status("/PortableApp.zip", 500, "server error");

    let name = "Contoso.ServerErrorApp";
    let version = "1.0.0";
    let source = Source {
        url: format!("{}/PortableApp.zip", server.url()),
        checksum: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string(),
        kind: "portable".to_string(),
    };

    let install_root = install_root();
    let install_dir = paths::package_dir_at(&install_root, name);
    let cache_file = paths::cache_file_at(&install_root, name, version, "zip");

    let context = InstallPlan {
        name: name.to_string(),
        package_version: version.to_string(),
        source,
        cache_file: cache_file.clone(),
        install_dir: install_dir.clone(),
        backup_dir: install_dir.with_extension("backup"),
        product_code: None,
        dependencies: vec![],
    };

    let conn = database::get_conn()?;
    let mut progress_events = Vec::new();

    let error = portable::install(&conn, &context, &mut |current, total| {
        progress_events.push((current, total));
    })
    .expect_err("server error should fail");

    error_mock.assert();
    assert!(
        error
            .to_string()
            .contains("download and verification failed")
    );
    assert!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("server returned error"))
    );
    assert!(!cache_file.with_extension("part").exists());
    assert!(!install_dir.exists());
    assert!(progress_events.is_empty());

    let package =
        database::get_package(&conn, name)?.expect("failed install should still be tracked");
    assert_eq!(package.status, PackageStatus::Failed);
    assert_eq!(package.version, version);
    assert_eq!(package.kind, "portable");

    Ok(())
}

#[test]
fn portable_install_propagates_rate_limit_and_cleans_up() -> Result<()> {
    let _guard = env_lock();
    let temp_root = tempdir()?;
    let _root_env = TestEnvVar::set("WINBREW_ROOT", temp_root.path().to_string_lossy().as_ref());

    fixtures::init_database_root(temp_root.path())?;

    let mut server = MockServer::new();
    let rate_limit_mock = server.get_text_status("/PortableApp.zip", 429, "rate limited");

    let name = "Contoso.RateLimitedApp";
    let version = "1.0.0";
    let source = Source {
        url: format!("{}/PortableApp.zip", server.url()),
        checksum: "ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff".to_string(),
        kind: "portable".to_string(),
    };

    let install_root = install_root();
    let install_dir = paths::package_dir_at(&install_root, name);
    let cache_file = paths::cache_file_at(&install_root, name, version, "zip");

    let context = InstallPlan {
        name: name.to_string(),
        package_version: version.to_string(),
        source,
        cache_file: cache_file.clone(),
        install_dir: install_dir.clone(),
        backup_dir: install_dir.with_extension("backup"),
        product_code: None,
        dependencies: vec![],
    };

    let conn = database::get_conn()?;
    let mut progress_events = Vec::new();

    let error = portable::install(&conn, &context, &mut |current, total| {
        progress_events.push((current, total));
    })
    .expect_err("rate limit should fail");

    rate_limit_mock.assert();
    assert!(
        error
            .to_string()
            .contains("download and verification failed")
    );
    assert!(
        error
            .chain()
            .any(|cause| cause.to_string().contains("server returned error"))
    );
    assert!(!cache_file.with_extension("part").exists());
    assert!(!install_dir.exists());
    assert!(progress_events.is_empty());

    let package =
        database::get_package(&conn, name)?.expect("failed install should still be tracked");
    assert_eq!(package.status, PackageStatus::Failed);
    assert_eq!(package.version, version);
    assert_eq!(package.kind, "portable");

    Ok(())
}
