use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, params};

use crate::error::ParserError;
use crate::parser::ParsedPackage;

const PACKAGE_UPSERT: &str = r#"
INSERT INTO catalog_packages(id, name, version, source, namespace, source_id, created_at, updated_at, description, homepage, license, publisher, locale, moniker, platform, commands, protocols, file_extensions, capabilities, tags, bin)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)
ON CONFLICT(id) DO UPDATE SET
    name=excluded.name,
    version=excluded.version,
    source=excluded.source,
    namespace=excluded.namespace,
    source_id=excluded.source_id,
    updated_at=CURRENT_TIMESTAMP,
    description=excluded.description,
    homepage=excluded.homepage,
    license=excluded.license,
    publisher=excluded.publisher,
    locale=excluded.locale,
    moniker=excluded.moniker,
    platform=excluded.platform,
    commands=excluded.commands,
    protocols=excluded.protocols,
    file_extensions=excluded.file_extensions,
    capabilities=excluded.capabilities,
    tags=excluded.tags,
    bin=excluded.bin
"#;

const RAW_UPSERT: &str = r#"
INSERT INTO catalog_packages_raw(package_id, raw)
VALUES (?1, ?2)
ON CONFLICT(package_id) DO UPDATE SET
    raw=excluded.raw
"#;

const DELETE_INSTALLERS: &str = "DELETE FROM catalog_installers WHERE package_id = ?1";

const INSTALLER_INSERT: &str = r#"
INSERT INTO catalog_installers(package_id, url, hash, hash_algorithm, installer_type, installer_switches, platform, commands, protocols, file_extensions, capabilities, scope, arch, kind, nested_kind)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
"#;

const SCHEMA: &str = include_str!("../schema/catalog.sql");

pub struct CatalogWriter {
    catalog_db_path: PathBuf,
    connection: Connection,
    committed: bool,
}

impl CatalogWriter {
    pub fn open(path: &Path) -> Result<Self, ParserError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let catalog_db_path = path.to_path_buf();

        let mut cleanup_paths = Vec::with_capacity(3);
        cleanup_paths.push(path.to_path_buf());
        cleanup_paths.push(PathBuf::from(format!("{}-wal", path.display())));
        cleanup_paths.push(PathBuf::from(format!("{}-shm", path.display())));

        for cleanup_path in cleanup_paths {
            let _ = fs::remove_file(cleanup_path);
        }

        if path.exists() {
            fs::remove_file(path)?;
        }

        let connection = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_CREATE,
        )
        .map_err(|source| ParserError::from((catalog_db_path.clone(), source)))?;
        connection
            .execute_batch(
                "PRAGMA foreign_keys=ON; PRAGMA journal_mode=DELETE; PRAGMA synchronous=NORMAL; PRAGMA cache_size=-2000; PRAGMA temp_store=MEMORY; BEGIN IMMEDIATE;",
            )
            .map_err(|source| ParserError::from((catalog_db_path.clone(), source)))?;
        connection
            .execute_batch(SCHEMA)
            .map_err(|source| ParserError::from((catalog_db_path.clone(), source)))?;

        Ok(Self {
            catalog_db_path,
            connection,
            committed: false,
        })
    }

    pub fn write_package(&mut self, parsed: &ParsedPackage) -> Result<(), ParserError> {
        let mut package_stmt = self
            .connection
            .prepare(PACKAGE_UPSERT)
            .map_err(|source| ParserError::from((self.catalog_db_path.clone(), source)))?;
        let mut raw_stmt = self
            .connection
            .prepare(RAW_UPSERT)
            .map_err(|source| ParserError::from((self.catalog_db_path.clone(), source)))?;
        let mut delete_installers_stmt = self
            .connection
            .prepare(DELETE_INSTALLERS)
            .map_err(|source| ParserError::from((self.catalog_db_path.clone(), source)))?;
        let mut installer_stmt = self
            .connection
            .prepare(INSTALLER_INSERT)
            .map_err(|source| ParserError::from((self.catalog_db_path.clone(), source)))?;

        package_stmt
            .execute(params![
                parsed.package.id.as_str(),
                parsed.package.name.as_str(),
                parsed.package.version.to_string(),
                parsed.package.source.as_str(),
                parsed.package.namespace.as_deref(),
                parsed.package.source_id.as_str(),
                parsed.package.description.as_deref(),
                parsed.package.homepage.as_deref(),
                parsed.package.license.as_deref(),
                parsed.package.publisher.as_deref(),
                parsed.package.locale.as_deref(),
                parsed.package.moniker.as_deref(),
                parsed.package.platform.as_deref(),
                parsed.package.commands.as_deref(),
                parsed.package.protocols.as_deref(),
                parsed.package.file_extensions.as_deref(),
                parsed.package.capabilities.as_deref(),
                parsed.package.tags.as_deref(),
                parsed.package.bin.as_deref(),
            ])
            .map_err(|source| ParserError::from((self.catalog_db_path.clone(), source)))?;

        raw_stmt
            .execute(params![
                parsed.package.id.as_str(),
                parsed.raw_json.as_str()
            ])
            .map_err(|source| ParserError::from((self.catalog_db_path.clone(), source)))?;
        delete_installers_stmt
            .execute(params![parsed.package.id.as_str()])
            .map_err(|source| ParserError::from((self.catalog_db_path.clone(), source)))?;

        let mut installers: Vec<_> = parsed.installers.iter().collect();
        installers.sort_by(|left, right| {
            left.url
                .cmp(&right.url)
                .then(left.hash.cmp(&right.hash))
                .then(
                    left.hash_algorithm
                        .as_str()
                        .cmp(right.hash_algorithm.as_str()),
                )
                .then(
                    left.installer_type
                        .as_str()
                        .cmp(right.installer_type.as_str()),
                )
                .then(
                    left.installer_switches
                        .as_deref()
                        .cmp(&right.installer_switches.as_deref()),
                )
                .then(left.scope.as_deref().cmp(&right.scope.as_deref()))
                .then(left.arch.as_str().cmp(right.arch.as_str()))
                .then(left.kind.as_str().cmp(right.kind.as_str()))
                .then(
                    left.nested_kind
                        .map(|kind| kind.as_str())
                        .cmp(&right.nested_kind.map(|kind| kind.as_str())),
                )
        });

        for installer in installers {
            let hash = if installer.hash.trim().is_empty() {
                None
            } else {
                Some(installer.hash.as_str())
            };

            installer_stmt
                .execute(params![
                    parsed.package.id.as_str(),
                    installer.url.as_str(),
                    hash,
                    installer.hash_algorithm.as_str(),
                    installer.installer_type.as_str(),
                    installer.installer_switches.as_deref(),
                    installer.platform.as_deref(),
                    installer.commands.as_deref(),
                    installer.protocols.as_deref(),
                    installer.file_extensions.as_deref(),
                    installer.capabilities.as_deref(),
                    installer.scope.as_deref(),
                    installer.arch.to_string(),
                    installer.kind.to_string(),
                    installer.nested_kind.map(|kind| kind.as_str()),
                ])
                .map_err(|source| ParserError::from((self.catalog_db_path.clone(), source)))?;
        }

        Ok(())
    }

    pub fn finish(mut self) -> Result<(), ParserError> {
        self.connection
            .execute_batch("COMMIT;")
            .map_err(|source| ParserError::from((self.catalog_db_path.clone(), source)))?;
        self.committed = true;
        Ok(())
    }
}

impl Drop for CatalogWriter {
    fn drop(&mut self) {
        if !self.committed {
            let _ = self.connection.execute_batch("ROLLBACK;");
        }
    }
}
