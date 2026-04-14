use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, params};

use crate::error::ParserError;
use crate::parser::ParsedPackage;

const PACKAGE_UPSERT: &str = r#"
INSERT INTO catalog_packages(id, name, version, source, namespace, source_id, description, homepage, license, publisher)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
ON CONFLICT(id) DO UPDATE SET
    name=excluded.name,
    version=excluded.version,
    source=excluded.source,
    namespace=excluded.namespace,
    source_id=excluded.source_id,
    description=excluded.description,
    homepage=excluded.homepage,
    license=excluded.license,
    publisher=excluded.publisher
"#;

const RAW_UPSERT: &str = r#"
INSERT INTO catalog_packages_raw(package_id, raw)
VALUES (?1, ?2)
ON CONFLICT(package_id) DO UPDATE SET
    raw=excluded.raw
"#;

const DELETE_INSTALLERS: &str = "DELETE FROM catalog_installers WHERE package_id = ?1";

const INSTALLER_INSERT: &str = r#"
INSERT INTO catalog_installers(package_id, url, hash, arch, type, nested_kind)
VALUES (?1, ?2, ?3, ?4, ?5, ?6)
"#;

const SCHEMA: &str = include_str!("../schema/catalog.sql");

pub struct CatalogWriter {
    connection: Connection,
    committed: bool,
}

impl CatalogWriter {
    pub fn open(path: &Path) -> Result<Self, ParserError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

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
        )?;
        connection.execute_batch(
            "PRAGMA foreign_keys=ON; PRAGMA journal_mode=DELETE; PRAGMA synchronous=NORMAL; PRAGMA cache_size=-2000; PRAGMA temp_store=MEMORY; BEGIN IMMEDIATE;",
        )?;
        connection.execute_batch(SCHEMA)?;

        Ok(Self {
            connection,
            committed: false,
        })
    }

    pub fn write_package(&mut self, parsed: &ParsedPackage) -> Result<(), ParserError> {
        let mut package_stmt = self.connection.prepare(PACKAGE_UPSERT)?;
        let mut raw_stmt = self.connection.prepare(RAW_UPSERT)?;
        let mut delete_installers_stmt = self.connection.prepare(DELETE_INSTALLERS)?;
        let mut installer_stmt = self.connection.prepare(INSTALLER_INSERT)?;

        package_stmt.execute(params![
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
        ])?;

        raw_stmt.execute(params![
            parsed.package.id.as_str(),
            parsed.raw_json.as_str()
        ])?;
        delete_installers_stmt.execute(params![parsed.package.id.as_str()])?;

        let mut installers: Vec<_> = parsed.installers.iter().collect();
        installers.sort_by(|left, right| {
            left.url
                .cmp(&right.url)
                .then(left.hash.cmp(&right.hash))
                .then(left.arch.as_str().cmp(right.arch.as_str()))
                .then(left.kind.as_str().cmp(right.kind.as_str()))
                .then(
                    left.nested_kind
                        .map(|kind| kind.as_str())
                        .cmp(&right.nested_kind.map(|kind| kind.as_str())),
                )
        });

        for installer in installers {
            installer_stmt.execute(params![
                parsed.package.id.as_str(),
                installer.url.as_str(),
                installer.hash.as_str(),
                installer.arch.to_string(),
                installer.kind.to_string(),
                installer.nested_kind.map(|kind| kind.as_str()),
            ])?;
        }

        Ok(())
    }

    pub fn finish(mut self) -> Result<(), ParserError> {
        self.connection.execute_batch("COMMIT;")?;
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
