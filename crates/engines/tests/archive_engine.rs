use std::fs;
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use tempfile::tempdir;
use winbrew_engines::{
    DeploymentKind, EngineKind, PackageEngine, probe_installer_from_download,
    resolve_deployment_kind, resolve_engine_for_installer,
};
use winbrew_models::catalog::{CatalogInstaller, CatalogInstallerType};
use winbrew_models::install::{Architecture, InstalledPackage, InstallerType, PackageStatus};
use winbrew_models::package::PackageSource;
use winbrew_models::shared::{CatalogId, HashAlgorithm};
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

fn catalog_installer(kind: InstallerType, file_name: &str) -> CatalogInstaller {
    let url = format!("https://example.invalid/{file_name}");

    CatalogInstaller {
        package_id: CatalogId::parse("winget/Contoso.App").expect("catalog id"),
        url: url.clone(),
        hash: String::new(),
        hash_algorithm: HashAlgorithm::default(),
        installer_type: CatalogInstallerType::normalize(PackageSource::Winget, kind, &url),
        installer_switches: None,
        platform: None,
        commands: None,
        protocols: None,
        file_extensions: None,
        capabilities: None,
        arch: Architecture::Any,
        kind,
        nested_kind: None,
        scope: None,
    }
}

fn installed_package(
    install_dir: &Path,
    kind: InstallerType,
    engine_kind: EngineKind,
    deployment_kind: DeploymentKind,
) -> InstalledPackage {
    InstalledPackage {
        name: "Contoso.App".to_string(),
        version: "1.0.0".to_string(),
        kind,
        deployment_kind,
        engine_kind,
        engine_metadata: None,
        install_dir: install_dir.to_string_lossy().into_owned(),
        dependencies: Vec::new(),
        status: PackageStatus::Ok,
        installed_at: "2026-05-14T00:00:00Z".to_string(),
    }
}

fn create_zip_archive(path: &Path, entries: &[(&str, &[u8])]) -> Result<()> {
    let file = fs::File::create(path)?;
    let mut writer = ZipWriter::new(file);

    for (file_name, contents) in entries {
        writer.start_file(*file_name, SimpleFileOptions::default())?;
        writer.write_all(contents)?;
    }

    writer.finish()?;
    Ok(())
}

#[test]
fn archive_engine_extracts_zip_and_removes_directory() -> Result<()> {
    let temp_root = tempdir()?;
    let download_path = temp_root.path().join("payload.zip");
    let install_dir = temp_root.path().join("packages").join("Contoso.Archive");

    create_zip_archive(&download_path, &[("bin/tool.exe", b"archive-binary")])?;

    let installer = catalog_installer(InstallerType::Portable, "payload.zip");
    let resolved_kind = probe_installer_from_download(&installer, &download_path)?;
    assert_eq!(resolved_kind, InstallerType::Zip);

    let mut resolved_installer = installer.clone();
    resolved_installer.kind = resolved_kind;

    let engine = resolve_engine_for_installer(&resolved_installer)?;
    assert_eq!(engine, EngineKind::Zip);

    let receipt = engine.install(
        &resolved_installer,
        &download_path,
        &install_dir,
        "Contoso.Archive",
    )?;

    assert_eq!(receipt.engine_kind, EngineKind::Zip);
    assert_eq!(receipt.install_dir, install_dir.to_string_lossy());
    assert!(install_dir.join("bin").join("tool.exe").exists());
    assert_eq!(
        fs::read(install_dir.join("bin").join("tool.exe"))?,
        b"archive-binary"
    );

    let package = installed_package(
        &install_dir,
        resolved_installer.kind,
        engine,
        resolve_deployment_kind(&resolved_installer),
    );

    engine.remove(&package)?;

    assert!(!install_dir.exists());
    Ok(())
}
