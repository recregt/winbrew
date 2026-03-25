use anyhow::Result;
use sha2::{Digest, Sha256};
use tempfile::tempdir;
use winbrew::core::install::{InstallPlan, install_root};
use winbrew::core::paths;
use winbrew::database;
use winbrew::manifest::Source;
use winbrew::models::PackageStatus;
use winbrew::services::install::portable;

struct TestEnvVar {
    key: &'static str,
}

impl TestEnvVar {
    fn set(key: &'static str, value: &str) -> Self {
        unsafe {
            std::env::set_var(key, value);
        }

        Self { key }
    }
}

impl Drop for TestEnvVar {
    fn drop(&mut self) {
        unsafe {
            std::env::remove_var(self.key);
        }
    }
}

#[test]
fn portable_install_downloads_from_mock_server_and_updates_database() -> Result<()> {
    let temp_root = tempdir()?;
    let _root_env = TestEnvVar::set("WINBREW_ROOT", temp_root.path().to_string_lossy().as_ref());

    database::config_set("paths.root", &temp_root.path().to_string_lossy())?;

    let mut server = mockito::Server::new();
    let body = b"portable package contents";
    let checksum = hex::encode(Sha256::digest(body));
    let mock = server
        .mock("GET", "/PortableApp.zip")
        .with_status(200)
        .with_header("content-length", body.len().to_string().as_str())
        .with_body(body.as_ref())
        .create();

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
    assert_eq!(std::fs::read(&install_file)?, body);
    assert_eq!(std::fs::read(&cache_file)?, body);

    let package = database::get_package(&conn, name)?.expect("installed package should exist");
    assert_eq!(package.status, PackageStatus::Ok);
    assert_eq!(package.version, version);
    assert_eq!(package.kind, "portable");
    assert_eq!(package.dependencies, vec!["Example.Dependency".to_string()]);
    assert!(!progress_events.is_empty());

    Ok(())
}
