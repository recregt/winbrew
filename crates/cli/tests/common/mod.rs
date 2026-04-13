#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use rusqlite::{Connection, params};
use winbrew_cli::database::{self, Config};
use winbrew_cli::models::domains::install::{EngineKind, EngineMetadata, InstallerType};
use winbrew_cli::models::domains::installed::{InstalledPackage, PackageStatus};

pub const DEFAULT_INSTALLED_AT: &str = "2026-04-12T00:00:00Z";

pub fn test_root() -> tempfile::TempDir {
    tempfile::tempdir().expect("failed to create test root")
}

pub fn init_database(root: &std::path::Path) -> anyhow::Result<Config> {
    let config = database::Config::load_at(root)?;
    database::init(&config.resolved_paths())?;
    Ok(config)
}

pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("cli crate should live under crates/")
        .to_path_buf()
}

pub fn run_winbrew(root: &Path, args: &[&str]) -> Output {
    Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--locked")
        .arg("--manifest-path")
        .arg(repo_root().join("Cargo.toml"))
        .arg("-p")
        .arg("winbrew-bin")
        .arg("--bin")
        .arg("winbrew")
        .arg("--")
        .args(args)
        .env("WINBREW_PATHS_ROOT", root)
        .env("NO_COLOR", "1")
        .current_dir(repo_root())
        .output()
        .expect("failed to run winbrew binary")
}

pub fn output_text(output: &Output) -> String {
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    text
}

pub fn assert_success(output: &Output, context: &str) -> anyhow::Result<()> {
    anyhow::ensure!(
        output.status.success(),
        "{context} failed\nstatus: {}\nstdout:\n{}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr),
    );
    Ok(())
}

pub fn assert_output_contains(output: &Output, expected: &str) -> anyhow::Result<()> {
    let text = output_text(output);
    anyhow::ensure!(
        text.contains(expected),
        "Expected output to contain: {expected}\nActual output:\n{text}"
    );
    Ok(())
}

pub fn assert_output_contains_all(output: &Output, expected: &[&str]) -> anyhow::Result<()> {
    let text = output_text(output);
    for pattern in expected {
        anyhow::ensure!(
            text.contains(pattern),
            "Expected output to contain: {pattern}\nActual output:\n{text}"
        );
    }
    Ok(())
}

pub fn catalog_package_id(package_name: &str) -> String {
    format!("winget/{}", package_name.replace(' ', "."))
}

pub fn seed_catalog_package(
    conn: &Connection,
    package_name: &str,
    description: &str,
    installer_url: &str,
    hash: &str,
) -> anyhow::Result<()> {
    conn.execute_batch(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../tests/fixtures/catalog_schema.sql"
    )))?;

    conn.execute("DELETE FROM catalog_installers", [])?;
    conn.execute("DELETE FROM catalog_packages", [])?;

    let package_id = catalog_package_id(package_name);

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
            Some(description),
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

pub struct InstalledPackageBuilder {
    name: String,
    version: String,
    kind: InstallerType,
    status: PackageStatus,
    installed_at: String,
    dependencies: Vec<String>,
    engine_metadata: Option<EngineMetadata>,
}

impl InstalledPackageBuilder {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            version: "1.0.0".to_string(),
            kind: InstallerType::Portable,
            status: PackageStatus::Ok,
            installed_at: DEFAULT_INSTALLED_AT.to_string(),
            dependencies: Vec::new(),
            engine_metadata: None,
        }
    }

    pub fn version(mut self, version: impl Into<String>) -> Self {
        self.version = version.into();
        self
    }

    pub fn kind(mut self, kind: InstallerType) -> Self {
        self.kind = kind;
        self
    }

    pub fn status(mut self, status: PackageStatus) -> Self {
        self.status = status;
        self
    }

    pub fn installed_at(mut self, installed_at: impl Into<String>) -> Self {
        self.installed_at = installed_at.into();
        self
    }

    pub fn dependencies(mut self, dependencies: Vec<String>) -> Self {
        self.dependencies = dependencies;
        self
    }

    pub fn engine_metadata(mut self, engine_metadata: Option<EngineMetadata>) -> Self {
        self.engine_metadata = engine_metadata;
        self
    }

    pub fn build(self, install_dir: &Path) -> InstalledPackage {
        InstalledPackage {
            name: self.name,
            version: self.version,
            kind: self.kind,
            deployment_kind: self.kind.deployment_kind(),
            engine_kind: EngineKind::from(self.kind),
            engine_metadata: self.engine_metadata,
            install_dir: install_dir.to_string_lossy().to_string(),
            dependencies: self.dependencies,
            status: self.status,
            installed_at: self.installed_at,
        }
    }
}
