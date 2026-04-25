use crate::core::hash::hash_algorithm;
use crate::models::domains::catalog::{CatalogInstaller, CatalogInstallerType, CatalogPackage};
use crate::models::domains::install::{Architecture, InstallerType};
use crate::models::domains::package::{PackageId, PackageSource};
use crate::models::domains::shared::{CatalogId, HashAlgorithm as CatalogHashAlgorithm, Version};
use anyhow::Result;
use rusqlite::{Connection, params};
use std::path::Path;

#[derive(Debug, Clone, Copy)]
pub struct CatalogInstallerSeed<'a> {
    pub url: &'a str,
    pub hash: &'a str,
    pub kind: InstallerType,
    pub installer_switches: Option<&'a str>,
    pub arch: Architecture,
    pub platform: Option<&'a str>,
    pub scope: Option<&'a str>,
}

pub fn catalog_package_id(package_name: &str) -> String {
    format!("winget/{}", package_name.replace(' ', "."))
}

pub fn catalog_installer(package_id: CatalogId, url: &str) -> CatalogInstaller {
    CatalogInstaller {
        package_id,
        url: url.to_string(),
        hash: "abc123".to_string(),
        hash_algorithm: CatalogHashAlgorithm::Sha256,
        installer_type: CatalogInstallerType::Unknown,
        installer_switches: None,
        platform: None,
        commands: None,
        protocols: None,
        file_extensions: None,
        capabilities: None,
        arch: Architecture::X64,
        kind: InstallerType::Exe,
        nested_kind: None,
        scope: None,
    }
}

pub trait CatalogInstallerBuilderExt {
    fn with_installer_type(self, installer_type: CatalogInstallerType) -> Self;
    fn with_installer_switches<T: Into<String>>(self, installer_switches: T) -> Self;
    fn with_hash_algorithm(self, hash_algorithm: CatalogHashAlgorithm) -> Self;
    fn with_hash<T: Into<String>>(self, hash: T) -> Self;
    fn with_arch(self, arch: Architecture) -> Self;
    fn with_kind(self, kind: InstallerType) -> Self;
    fn with_nested(self, nested_kind: InstallerType) -> Self;
    fn with_scope<T: Into<String>>(self, scope: T) -> Self;
}

impl CatalogInstallerBuilderExt for CatalogInstaller {
    fn with_installer_type(mut self, installer_type: CatalogInstallerType) -> Self {
        self.installer_type = installer_type;
        self
    }

    fn with_installer_switches<T: Into<String>>(mut self, installer_switches: T) -> Self {
        self.installer_switches = Some(installer_switches.into());
        self
    }

    fn with_hash_algorithm(mut self, hash_algorithm: CatalogHashAlgorithm) -> Self {
        self.hash_algorithm = hash_algorithm;
        self
    }

    fn with_hash<T: Into<String>>(mut self, hash: T) -> Self {
        self.hash = hash.into();
        self
    }

    fn with_arch(mut self, arch: Architecture) -> Self {
        self.arch = arch;
        self
    }

    fn with_kind(mut self, kind: InstallerType) -> Self {
        self.kind = kind;
        self
    }

    fn with_nested(mut self, nested_kind: InstallerType) -> Self {
        self.nested_kind = Some(nested_kind);
        self
    }

    fn with_scope<T: Into<String>>(mut self, scope: T) -> Self {
        self.scope = Some(scope.into());
        self
    }
}

pub fn catalog_package(id: CatalogId, name: &str, version: Version) -> CatalogPackage {
    let package_id = PackageId::parse(id.as_ref()).expect("catalog id should parse");

    CatalogPackage {
        id,
        name: name.to_string(),
        version,
        source: package_id.source(),
        namespace: package_id.namespace().map(str::to_string),
        source_id: package_id.source_id().to_string(),
        created_at: None,
        updated_at: None,
        description: None,
        homepage: None,
        license: None,
        publisher: None,
        locale: None,
        moniker: None,
        platform: None,
        commands: None,
        protocols: None,
        file_extensions: None,
        capabilities: None,
        tags: None,
        bin: None,
    }
}

pub trait CatalogPackageBuilderExt {
    fn with_source(self, source: PackageSource) -> Self;
    fn with_namespace<T: Into<String>>(self, namespace: T) -> Self;
    fn without_namespace(self) -> Self;
    fn with_source_id<T: Into<String>>(self, source_id: T) -> Self;
    fn with_created_at<T: Into<String>>(self, created_at: T) -> Self;
    fn with_updated_at<T: Into<String>>(self, updated_at: T) -> Self;
    fn with_description<T: Into<String>>(self, description: T) -> Self;
    fn with_homepage<T: Into<String>>(self, homepage: T) -> Self;
    fn with_license<T: Into<String>>(self, license: T) -> Self;
    fn with_publisher<T: Into<String>>(self, publisher: T) -> Self;
    fn with_locale<T: Into<String>>(self, locale: T) -> Self;
    fn with_moniker<T: Into<String>>(self, moniker: T) -> Self;
    fn with_tags<T: Into<String>>(self, tags: T) -> Self;
    fn with_bin<T: Into<String>>(self, bin: T) -> Self;
}

impl CatalogPackageBuilderExt for CatalogPackage {
    fn with_source(mut self, source: PackageSource) -> Self {
        self.source = source;
        self
    }

    fn with_namespace<T: Into<String>>(mut self, namespace: T) -> Self {
        self.namespace = Some(namespace.into());
        self
    }

    fn without_namespace(mut self) -> Self {
        self.namespace = None;
        self
    }

    fn with_source_id<T: Into<String>>(mut self, source_id: T) -> Self {
        self.source_id = source_id.into();
        self
    }

    fn with_created_at<T: Into<String>>(mut self, created_at: T) -> Self {
        self.created_at = Some(created_at.into());
        self
    }

    fn with_updated_at<T: Into<String>>(mut self, updated_at: T) -> Self {
        self.updated_at = Some(updated_at.into());
        self
    }

    fn with_description<T: Into<String>>(mut self, description: T) -> Self {
        self.description = Some(description.into());
        self
    }

    fn with_homepage<T: Into<String>>(mut self, homepage: T) -> Self {
        self.homepage = Some(homepage.into());
        self
    }

    fn with_license<T: Into<String>>(mut self, license: T) -> Self {
        self.license = Some(license.into());
        self
    }

    fn with_publisher<T: Into<String>>(mut self, publisher: T) -> Self {
        self.publisher = Some(publisher.into());
        self
    }

    fn with_locale<T: Into<String>>(mut self, locale: T) -> Self {
        self.locale = Some(locale.into());
        self
    }

    fn with_moniker<T: Into<String>>(mut self, moniker: T) -> Self {
        self.moniker = Some(moniker.into());
        self
    }

    fn with_tags<T: Into<String>>(mut self, tags: T) -> Self {
        self.tags = Some(tags.into());
        self
    }

    fn with_bin<T: Into<String>>(mut self, bin: T) -> Self {
        self.bin = Some(bin.into());
        self
    }
}

pub fn seed_catalog_package(
    conn: &Connection,
    package_name: &str,
    description: &str,
    installer_url: &str,
    hash: &str,
) -> Result<()> {
    seed_catalog_package_with_installer(
        conn,
        package_name,
        description,
        installer_url,
        hash,
        InstallerType::Zip,
        None,
    )
}

pub fn seed_catalog_db_with_installer(
    path: &Path,
    package_name: &str,
    description: &str,
    installer_url: &str,
    hash: &str,
    kind: InstallerType,
    installer_switches: Option<&str>,
) -> Result<()> {
    let conn = Connection::open(path)?;
    seed_catalog_package_with_installer(
        &conn,
        package_name,
        description,
        installer_url,
        hash,
        kind,
        installer_switches,
    )
}

pub fn seed_catalog_db_with_installers(
    path: &Path,
    package_name: &str,
    description: &str,
    installers: &[CatalogInstallerSeed<'_>],
) -> Result<()> {
    let conn = Connection::open(path)?;
    seed_catalog_package_with_installers(&conn, package_name, description, installers)
}

pub fn append_catalog_db(
    path: &Path,
    package_name: &str,
    description: &str,
    installer_url: &str,
    hash: &str,
) -> Result<()> {
    append_catalog_db_with_installer(
        path,
        package_name,
        description,
        installer_url,
        hash,
        InstallerType::Zip,
        None,
    )
}

pub fn append_catalog_db_with_installer(
    path: &Path,
    package_name: &str,
    description: &str,
    installer_url: &str,
    hash: &str,
    kind: InstallerType,
    installer_switches: Option<&str>,
) -> Result<()> {
    let conn = Connection::open(path)?;
    insert_catalog_package(
        &conn,
        package_name,
        description,
        installer_url,
        hash,
        kind,
        installer_switches,
    )
}

fn insert_catalog_package(
    conn: &Connection,
    package_name: &str,
    description: &str,
    installer_url: &str,
    hash: &str,
    kind: InstallerType,
    installer_switches: Option<&str>,
) -> Result<()> {
    let package_id = catalog_package_id(package_name);
    let source_id = package_id
        .split_once('/')
        .map(|(_, source_id)| source_id.to_string())
        .unwrap_or_else(|| package_id.clone());
    let installer_type =
        CatalogInstallerType::normalize(PackageSource::Winget, kind, installer_url);

    conn.execute(
        r#"
        INSERT INTO catalog_packages (
            id, name, version, source, namespace, source_id, description, homepage, license, publisher, locale
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
        "#,
        params![
            package_id.clone(),
            package_name,
            "1.0.0",
            "winget",
            Option::<String>::None,
            source_id,
            Some(description),
            Option::<String>::None,
            Option::<String>::None,
            Some("Winbrew Ltd."),
            Some("en-US"),
        ],
    )?;

    conn.execute(
        r#"
        INSERT INTO catalog_installers (
            package_id, url, hash, hash_algorithm, installer_type, installer_switches, arch, kind
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
        "#,
        params![
            package_id,
            installer_url,
            if hash.trim().is_empty() {
                Option::<String>::None
            } else {
                Some(hash.to_string())
            },
            hash_algorithm(hash)
                .unwrap_or(CatalogHashAlgorithm::Sha256)
                .as_str(),
            installer_type.as_str(),
            installer_switches.map(|value| value.to_string()),
            "",
            kind.as_str(),
        ],
    )?;

    Ok(())
}

pub fn seed_catalog_db(
    path: &Path,
    package_name: &str,
    description: &str,
    installer_url: &str,
    hash: &str,
) -> Result<()> {
    seed_catalog_db_with_installer(
        path,
        package_name,
        description,
        installer_url,
        hash,
        InstallerType::Zip,
        None,
    )
}

pub fn seed_catalog_package_with_installer(
    conn: &Connection,
    package_name: &str,
    description: &str,
    installer_url: &str,
    hash: &str,
    kind: InstallerType,
    installer_switches: Option<&str>,
) -> Result<()> {
    conn.execute_batch(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../infra/parser/schema/catalog.sql"
    )))?;

    conn.execute("DELETE FROM catalog_installers", [])?;
    conn.execute("DELETE FROM catalog_packages", [])?;

    insert_catalog_package(
        conn,
        package_name,
        description,
        installer_url,
        hash,
        kind,
        installer_switches,
    )
}

pub fn seed_catalog_package_with_installers(
    conn: &Connection,
    package_name: &str,
    description: &str,
    installers: &[CatalogInstallerSeed<'_>],
) -> Result<()> {
    conn.execute_batch(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../infra/parser/schema/catalog.sql"
    )))?;

    conn.execute("DELETE FROM catalog_installers", [])?;
    conn.execute("DELETE FROM catalog_packages", [])?;

    let package_id = catalog_package_id(package_name);
    let source_id = package_id
        .split_once('/')
        .map(|(_, source_id)| source_id.to_string())
        .unwrap_or_else(|| package_id.clone());

    conn.execute(
            r#"
            INSERT INTO catalog_packages (
                id, name, version, source, namespace, source_id, description, homepage, license, publisher, locale
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)
            "#,
            params![
                package_id.clone(),
                package_name,
                "1.0.0",
                "winget",
                Option::<String>::None,
                source_id,
                Some(description),
                Option::<String>::None,
                Option::<String>::None,
                Some("Winbrew Ltd."),
                Some("en-US"),
            ],
        )?;

    for installer in installers {
        insert_catalog_installer(conn, &package_id, installer)?;
    }

    Ok(())
}

fn insert_catalog_installer(
    conn: &Connection,
    package_id: &str,
    installer: &CatalogInstallerSeed<'_>,
) -> Result<()> {
    let installer_type =
        CatalogInstallerType::normalize(PackageSource::Winget, installer.kind, installer.url);

    conn.execute(
            r#"
            INSERT INTO catalog_installers (
                package_id, url, hash, hash_algorithm, installer_type, installer_switches, platform, scope, arch, kind
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            "#,
            params![
                package_id,
                installer.url,
                if installer.hash.trim().is_empty() {
                    Option::<String>::None
                } else {
                    Some(installer.hash.to_string())
                },
                hash_algorithm(installer.hash)
                    .unwrap_or(CatalogHashAlgorithm::Sha256)
                    .as_str(),
                installer_type.as_str(),
                installer.installer_switches.map(|value| value.to_string()),
                installer.platform.map(|value| value.to_string()),
                installer.scope.map(|value| value.to_string()),
                installer.arch.as_str(),
                installer.kind.as_str(),
            ],
        )?;

    Ok(())
}
