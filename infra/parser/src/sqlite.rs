use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, params};

use crate::error::ParserError;
use crate::parser::ParsedPackage;
use winbrew_models::catalog::CanonicalInstallerKey;
use winbrew_models::catalog::metadata::CATALOG_DB_SCHEMA_VERSION;
use winbrew_models::catalog::package::CatalogInstaller;

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

const INSTALLER_INSERT: &str = r#"
INSERT INTO catalog_installers(package_id, url, hash, hash_algorithm, installer_type, installer_switches, platform, commands, protocols, file_extensions, capabilities, scope, arch, kind, nested_kind)
VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15)
"#;

const INSTALLER_SELECT_EXISTING: &str = r#"
SELECT id, url, hash, hash_algorithm, installer_type, installer_switches, scope, arch, kind, nested_kind
FROM catalog_installers
WHERE package_id = ?1
"#;

const INSTALLER_UPDATE_METADATA: &str = r#"
UPDATE catalog_installers
SET platform = ?2, commands = ?3, protocols = ?4, file_extensions = ?5, capabilities = ?6
WHERE id = ?1
"#;

const INSTALLER_DELETE_BY_ID: &str = "DELETE FROM catalog_installers WHERE id = ?1";

const SEEN_PACKAGE_INSERT: &str = "INSERT OR IGNORE INTO _parser_seen_packages(id) VALUES (?1)";

const PRUNE_STALE_PACKAGES: &str =
    "DELETE FROM catalog_packages WHERE id NOT IN (SELECT id FROM _parser_seen_packages)";

const SCHEMA: &str = include_str!("../schema/catalog.sql");

pub struct CatalogWriter {
    catalog_db_path: PathBuf,
    connection: Connection,
    committed: bool,
}

impl CatalogWriter {
    /// Open the catalog database for a materialization run.
    ///
    /// An existing, schema-compatible catalog at `path` is reused and written
    /// incrementally: packages and installers are upserted in place, and only
    /// rows that no longer appear in this run are pruned at `finish`. This
    /// keeps SQLite `rowid`/`id` values stable across runs for unchanged rows,
    /// which is what makes package-level delta patches safe to generate. A
    /// missing, unreadable, or schema-incompatible catalog falls back to a
    /// fresh rebuild, matching the previous always-destructive behavior.
    pub fn open(path: &Path) -> Result<Self, ParserError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let catalog_db_path = path.to_path_buf();

        // Stray WAL/SHM files can only be left behind by an interrupted run,
        // since this connection always runs in journal_mode=DELETE; clear
        // them defensively before deciding whether the main file is reusable.
        for stray in [
            PathBuf::from(format!("{}-wal", path.display())),
            PathBuf::from(format!("{}-shm", path.display())),
        ] {
            let _ = fs::remove_file(stray);
        }

        if path.exists() && !catalog_is_reusable(path) {
            eprintln!(
                "[parser] existing catalog at {} is missing, unreadable, or has an incompatible schema version; rebuilding from scratch",
                path.display()
            );
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
        connection
            .execute_batch("CREATE TEMP TABLE _parser_seen_packages(id TEXT PRIMARY KEY);")
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
        let mut seen_stmt = self
            .connection
            .prepare(SEEN_PACKAGE_INSERT)
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

        seen_stmt
            .execute(params![parsed.package.id.as_str()])
            .map_err(|source| ParserError::from((self.catalog_db_path.clone(), source)))?;

        let installers = merge_installers(&parsed.installers)?;
        sync_installers(
            &self.connection,
            &self.catalog_db_path,
            parsed.package.id.as_str(),
            &installers,
        )?;

        Ok(())
    }

    pub fn finish(mut self) -> Result<(), ParserError> {
        self.connection
            .execute_batch(PRUNE_STALE_PACKAGES)
            .map_err(|source| ParserError::from((self.catalog_db_path.clone(), source)))?;
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

/// Return `true` when the catalog at `path` can be opened read-only and its
/// recorded schema version matches what this parser writes. Any failure
/// (missing table, unreadable file, mismatched version) means the caller
/// should discard the file and rebuild from scratch instead of upserting
/// into a shape it doesn't understand.
fn catalog_is_reusable(path: &Path) -> bool {
    let Ok(connection) = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)
    else {
        return false;
    };

    let version: Result<String, _> = connection.query_row(
        "SELECT value FROM schema_meta WHERE name = 'schema_version'",
        [],
        |row| row.get(0),
    );

    matches!(version, Ok(value) if value == CATALOG_DB_SCHEMA_VERSION.to_string())
}

/// Upsert `installers` for `package_id` in place, preserving each existing
/// installer's `id` when its canonical identity (url/hash/type/arch/kind/...)
/// is unchanged, and removing rows that are no longer present in this run.
///
/// This intentionally avoids the previous "delete all installers for the
/// package, then insert fresh rows" strategy: that reassigned a new
/// autoincrement `id` to every installer on every run, which is exactly the
/// kind of row-identity churn that makes downstream delta patches unsafe.
fn sync_installers(
    connection: &Connection,
    catalog_db_path: &Path,
    package_id: &str,
    installers: &[CatalogInstaller],
) -> Result<(), ParserError> {
    let mut existing: HashMap<CanonicalInstallerKey, i64> = HashMap::new();
    {
        let mut select_stmt = connection
            .prepare(INSTALLER_SELECT_EXISTING)
            .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?;
        let mut rows = select_stmt
            .query(params![package_id])
            .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?;

        while let Some(row) = rows
            .next()
            .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?
        {
            let id: i64 = row
                .get(0)
                .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?;
            let key = CanonicalInstallerKey {
                package_id: package_id.to_string(),
                url: row
                    .get(1)
                    .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?,
                hash: row
                    .get::<_, Option<String>>(2)
                    .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?
                    .unwrap_or_default(),
                hash_algorithm: row
                    .get(3)
                    .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?,
                installer_type: row
                    .get(4)
                    .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?,
                installer_switches: row
                    .get(5)
                    .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?,
                scope: row
                    .get(6)
                    .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?,
                arch: row
                    .get(7)
                    .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?,
                kind: row
                    .get(8)
                    .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?,
                nested_kind: row
                    .get(9)
                    .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?,
            };
            existing.insert(key, id);
        }
    }

    let mut insert_stmt = connection
        .prepare(INSTALLER_INSERT)
        .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?;
    let mut update_stmt = connection
        .prepare(INSTALLER_UPDATE_METADATA)
        .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?;

    for installer in installers {
        let key = installer.canonical_key();

        if let Some(existing_id) = existing.remove(&key) {
            update_stmt
                .execute(params![
                    existing_id,
                    installer.platform.as_deref(),
                    installer.commands.as_deref(),
                    installer.protocols.as_deref(),
                    installer.file_extensions.as_deref(),
                    installer.capabilities.as_deref(),
                ])
                .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?;
            continue;
        }

        let hash = if installer.hash.trim().is_empty() {
            None
        } else {
            Some(installer.hash.as_str())
        };

        insert_stmt
            .execute(params![
                package_id,
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
            .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?;
    }

    if !existing.is_empty() {
        let mut delete_stmt = connection
            .prepare(INSTALLER_DELETE_BY_ID)
            .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?;
        for stale_id in existing.values() {
            delete_stmt
                .execute(params![stale_id])
                .map_err(|source| ParserError::from((catalog_db_path.to_path_buf(), source)))?;
        }
    }

    Ok(())
}

fn merge_installers(
    installers: &[winbrew_models::catalog::package::CatalogInstaller],
) -> Result<Vec<winbrew_models::catalog::package::CatalogInstaller>, ParserError> {
    let mut merged_by_key: HashMap<
        CanonicalInstallerKey,
        winbrew_models::catalog::package::CatalogInstaller,
    > = HashMap::with_capacity(installers.len());

    for installer in installers {
        let key = installer.canonical_key();

        match merged_by_key.entry(key) {
            std::collections::hash_map::Entry::Vacant(entry) => {
                entry.insert(installer.clone());
            }
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                entry.get_mut().merge_metadata_from(installer)?;
            }
        }
    }

    let mut merged: Vec<_> = merged_by_key.into_values().collect();
    merged.sort_by(compare_installers);

    Ok(merged)
}

fn compare_installers(
    left: &winbrew_models::catalog::package::CatalogInstaller,
    right: &winbrew_models::catalog::package::CatalogInstaller,
) -> Ordering {
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
}

#[cfg(test)]
mod tests {
    use super::merge_installers;
    use winbrew_models::catalog::CatalogInstallerType;
    use winbrew_models::catalog::package::CatalogInstaller;
    use winbrew_models::install::{Architecture, InstallerType};
    use winbrew_models::shared::HashAlgorithm;

    fn installer(
        nested_kind: Option<InstallerType>,
        platform: Option<&str>,
        commands: Option<&str>,
    ) -> CatalogInstaller {
        let mut installer = CatalogInstaller {
            package_id: "winget/Contoso.App".into(),
            url: "https://example.test/app.zip".to_string(),
            hash: "sha256:deadbeef".to_string(),
            hash_algorithm: HashAlgorithm::Sha256,
            installer_type: CatalogInstallerType::Zip,
            installer_switches: None,
            platform: None,
            commands: None,
            protocols: None,
            file_extensions: None,
            capabilities: None,
            arch: Architecture::X64,
            kind: InstallerType::Zip,
            nested_kind: None,
            scope: Some("machine".to_string()),
        };

        installer.nested_kind = nested_kind;
        installer.platform = platform.map(|value| value.to_string());
        installer.commands = commands.map(|value| value.to_string());
        installer.protocols = None;
        installer.file_extensions = None;
        installer.capabilities = None;
        installer.scope = Some("machine".to_string());

        installer
    }

    #[test]
    fn merge_installers_unions_metadata_only_duplicates() {
        let mut left = installer(
            Some(InstallerType::Msi),
            Some("[\"Windows.Desktop\"]"),
            Some("[\"contoso\"]"),
        );
        let right = installer(
            Some(InstallerType::Msi),
            Some("[\"Windows.Server\", \"Windows.Desktop\"]"),
            Some("[\"contoso-server\", \"contoso\"]"),
        );

        left.merge_metadata_from(&right)
            .expect("merge should succeed");

        assert_eq!(
            left.platform.as_deref(),
            Some("[\"Windows.Desktop\",\"Windows.Server\"]")
        );
        assert_eq!(
            left.commands.as_deref(),
            Some("[\"contoso\",\"contoso-server\"]")
        );
    }

    #[test]
    fn merge_installers_keeps_distinct_nested_kind_separate() {
        let installers = vec![
            installer(None, Some("[\"Windows.Desktop\"]"), Some("[\"contoso\"]")),
            installer(
                Some(InstallerType::Msi),
                Some("[\"Windows.Desktop\"]"),
                Some("[\"contoso\"]"),
            ),
        ];

        let merged = merge_installers(&installers).expect("merge should succeed");

        assert_eq!(merged.len(), 2);
        assert_ne!(merged[0].nested_kind, merged[1].nested_kind);
    }

    #[test]
    fn merge_installers_preserves_missing_metadata_when_only_one_side_has_values() {
        let installers = vec![
            installer(Some(InstallerType::Msi), None, None),
            installer(
                Some(InstallerType::Msi),
                Some("[\"Windows.Desktop\"]"),
                None,
            ),
        ];

        let merged = merge_installers(&installers).expect("merge should succeed");

        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].platform.as_deref(), Some("[\"Windows.Desktop\"]"));
        assert_eq!(merged[0].commands, None);
    }
}
