use anyhow::{Context, Result};
use rusqlite::{Connection, params};

use crate::models::catalog::installer_type::CatalogInstallerType;
use crate::models::catalog::package::CatalogInstaller;
use crate::models::catalog::raw::RawCatalogInstaller;
use crate::models::shared::HashAlgorithm;

/// Returns all catalog installers for the given `package_id`.
///
/// Results are ordered by the canonical installer identity columns so the
/// downstream installer selector sees deterministic ties.
///
/// # Errors
///
/// Returns an error if SQLite query execution or row conversion fails.
pub fn get_installers(conn: &Connection, package_id: &str) -> Result<Vec<CatalogInstaller>> {
    let mut stmt = conn.prepare(
        "SELECT package_id, url, hash, hash_algorithm, installer_type, installer_switches, platform, commands, protocols, file_extensions, capabilities, scope, arch, kind, nested_kind
         FROM catalog_installers
         WHERE package_id = ?1
            ORDER BY url ASC, hash ASC, hash_algorithm ASC, installer_type ASC, installer_switches ASC, scope ASC, arch ASC, kind ASC, nested_kind ASC",
    )?;

    stmt.query_map(params![package_id], row_to_installer)?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to read catalog installer")
}

fn row_to_installer(row: &rusqlite::Row) -> rusqlite::Result<CatalogInstaller> {
    // Raw catalog rows normalize a missing checksum to an empty string.
    let hash = row.get::<_, Option<String>>("hash")?.unwrap_or_default();

    let raw = RawCatalogInstaller {
        package_id: row.get::<_, String>("package_id")?,
        url: row.get("url")?,
        hash,
        hash_algorithm: parse_text::<HashAlgorithm>(row.get::<_, String>("hash_algorithm")?)?,
        installer_type: parse_text::<CatalogInstallerType>(
            row.get::<_, String>("installer_type")?,
        )?,
        installer_switches: row.get("installer_switches")?,
        platform: row.get("platform")?,
        commands: row.get("commands")?,
        protocols: row.get("protocols")?,
        file_extensions: row.get("file_extensions")?,
        capabilities: row.get("capabilities")?,
        scope: row.get("scope")?,
        arch: row.get("arch")?,
        kind: row.get("kind")?,
        nested_kind: row.get("nested_kind")?,
    };

    CatalogInstaller::try_from(raw).map_err(conversion_err)
}

fn parse_text<T>(value: String) -> rusqlite::Result<T>
where
    T: std::str::FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    value.parse::<T>().map_err(conversion_err)
}

fn conversion_err<E>(err: E) -> rusqlite::Error
where
    E: std::error::Error + Send + Sync + 'static,
{
    // Column index is not surfaced in our error path; 0 is a conventional placeholder.
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
}

#[cfg(test)]
mod tests {
    use super::get_installers;
    use crate::models::catalog::CatalogInstallerType;
    use crate::models::install::installer::InstallerType;
    use crate::models::shared::HashAlgorithm;
    use rusqlite::{Connection, params};

    const CATALOG_SCHEMA: &str = include_str!("../../../../infra/parser/schema/catalog.sql");

    fn insert_catalog_package(conn: &Connection) {
        conn.execute(
            "INSERT INTO catalog_packages (id, name, version, source, source_id) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                "winget/Contoso.App",
                "Contoso App",
                "1.2.3",
                "winget",
                "Contoso.App",
            ],
        )
        .expect("seed catalog package");
    }

    fn open_test_db() -> Connection {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        conn.execute_batch(CATALOG_SCHEMA)
            .expect("load catalog schema");
        insert_catalog_package(&conn);
        conn
    }

    #[test]
    fn get_installers_reads_nested_kind_when_present() {
        let conn = open_test_db();

        conn.execute(
            r#"
            INSERT INTO catalog_installers (package_id, url, hash, hash_algorithm, installer_type, installer_switches, arch, kind, nested_kind)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                "winget/Contoso.App",
                "https://example.test/app-one.zip",
                "sha256:deadbeef",
                "sha256",
                "zip",
                Option::<String>::None,
                "x64",
                "zip",
                "portable",
            ],
        )
        .expect("insert portable installer");

        conn.execute(
            r#"
            INSERT INTO catalog_installers (package_id, url, hash, hash_algorithm, installer_type, installer_switches, arch, kind, nested_kind)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                "winget/Contoso.App",
                "https://example.test/app-two.zip",
                "sha256:deadbeef",
                "sha256",
                "zip",
                Option::<String>::None,
                "x64",
                "zip",
                "msi",
            ],
        )
        .expect("insert msi installer");

        let installers = get_installers(&conn, "winget/Contoso.App")
            .expect("catalog installers should load with nested kind");

        assert_eq!(installers.len(), 2);
        assert_eq!(installers[0].nested_kind, Some(InstallerType::Portable));
        assert_eq!(installers[1].nested_kind, Some(InstallerType::Msi));
        assert_eq!(installers[0].hash_algorithm, HashAlgorithm::Sha256);
        assert_eq!(installers[1].hash_algorithm, HashAlgorithm::Sha256);
        assert_eq!(installers[0].installer_type, CatalogInstallerType::Zip);
        assert_eq!(installers[1].installer_type, CatalogInstallerType::Zip);
    }

    #[test]
    fn get_installers_reads_null_hash_as_empty_string() {
        let conn = open_test_db();

        conn.execute(
            r#"
            INSERT INTO catalog_installers (package_id, url, hash, hash_algorithm, installer_type, installer_switches, arch, kind, nested_kind)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
            "#,
            params![
                "winget/Contoso.App",
                "https://example.test/app.exe",
                Option::<String>::None,
                "sha256",
                "exe",
                Option::<String>::None,
                "x64",
                "exe",
                Option::<String>::None,
            ],
        )
        .expect("insert checksumless installer");

        let installers = get_installers(&conn, "winget/Contoso.App")
            .expect("catalog installers should load with null hash");

        assert_eq!(installers.len(), 1);
        assert!(installers[0].hash.is_empty());
        assert_eq!(installers[0].kind, InstallerType::Exe);
    }
}
