use anyhow::{Context, Result};
use rusqlite::{Connection, params};

use winbrew_models::catalog::installer_type::CatalogInstallerType;
use winbrew_models::catalog::package::CatalogInstaller;
use winbrew_models::catalog::raw::RawCatalogInstaller;
use winbrew_models::shared::HashAlgorithm;

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
    let hash = row.get::<_, Option<String>>("hash")?.unwrap_or_default();

    let raw = RawCatalogInstaller {
        package_id: row.get::<_, String>("package_id")?,
        url: row.get("url")?,
        hash,
        hash_algorithm: row
            .get::<_, String>("hash_algorithm")?
            .parse::<HashAlgorithm>()
            .map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?,
        installer_type: row
            .get::<_, String>("installer_type")?
            .parse::<CatalogInstallerType>()
            .map_err(|err| {
                rusqlite::Error::FromSqlConversionFailure(
                    0,
                    rusqlite::types::Type::Text,
                    Box::new(err),
                )
            })?,
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

    CatalogInstaller::try_from(raw).map_err(|err| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(err))
    })
}

#[cfg(test)]
mod tests {
    use super::get_installers;
    use rusqlite::{Connection, params};
    use winbrew_models::catalog::CatalogInstallerType;
    use winbrew_models::install::installer::InstallerType;
    use winbrew_models::shared::HashAlgorithm;

    fn create_catalog_installers_table(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE catalog_installers (\n    id INTEGER PRIMARY KEY AUTOINCREMENT,\n    package_id TEXT NOT NULL,\n    url TEXT NOT NULL,\n    hash TEXT,\n    hash_algorithm TEXT NOT NULL DEFAULT 'sha256',\n    installer_type TEXT NOT NULL DEFAULT 'unknown',\n    installer_switches TEXT,\n    platform TEXT,\n    commands TEXT,\n    protocols TEXT,\n    file_extensions TEXT,\n    capabilities TEXT,\n    scope TEXT,\n    arch TEXT NOT NULL DEFAULT '',\n    kind TEXT NOT NULL DEFAULT '',\n    nested_kind TEXT\n);",
        )
        .expect("catalog installers table should be created");
    }

    #[test]
    fn get_installers_reads_nested_kind_when_present() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        create_catalog_installers_table(&conn);

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
        let conn = Connection::open_in_memory().expect("open in-memory database");
        create_catalog_installers_table(&conn);

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
