use anyhow::Result;
use rusqlite::Connection;

use crate::database;
use crate::models::{CatalogInstaller, CatalogPackage};

pub fn search_catalog_packages(conn: &Connection, query: &str) -> Result<Vec<CatalogPackage>> {
    // Catalog search entry point for the install service.
    // Currently delegates to the database layer directly; this is where
    // result ranking, normalization, or exact-match priority will live.
    database::search(conn, query)
}

pub fn select_installer(installers: &[CatalogInstaller]) -> Result<CatalogInstaller> {
    let current_arch = current_arch_name();

    installers
        .iter()
        .find(|installer| installer.arch.eq_ignore_ascii_case(current_arch))
        .cloned()
        .or_else(|| {
            installers
                .iter()
                .find(|installer| installer.arch.trim().is_empty())
                .cloned()
        })
        .or_else(|| installers.first().cloned())
        .ok_or_else(|| anyhow::anyhow!("catalog package has no installers"))
}

fn current_arch_name() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "x86" => "x86",
        "aarch64" => "arm64",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use rusqlite::{Connection, params};
    use tempfile::tempdir;

    fn sample_installer(arch: &str, kind: &str) -> CatalogInstaller {
        CatalogInstaller {
            package_id: "Contoso.App".to_string(),
            url: "https://example.test/app.exe".to_string(),
            hash: "sha256:deadbeef".to_string(),
            arch: arch.to_string(),
            kind: kind.to_string(),
        }
    }

    fn current_arch_alias() -> &'static str {
        match std::env::consts::ARCH {
            "x86_64" => "x64",
            "x86" => "x86",
            "aarch64" => "arm64",
            other => other,
        }
    }

    fn create_catalog_db() -> Result<(tempfile::TempDir, Connection)> {
        let temp_dir = tempdir()?;
        let db_path = temp_dir.path().join("catalog.db");
        let conn = Connection::open(db_path)?;

        conn.execute_batch(
            r#"
            CREATE TABLE catalog_packages (
                id          TEXT PRIMARY KEY,
                name        TEXT NOT NULL,
                version     TEXT NOT NULL,
                source      TEXT NOT NULL,
                description TEXT,
                homepage    TEXT,
                license     TEXT,
                publisher   TEXT
            );

            CREATE VIRTUAL TABLE catalog_packages_fts USING fts5(
                name,
                description,
                content=catalog_packages,
                content_rowid=rowid
            );

            CREATE TRIGGER catalog_packages_ai AFTER INSERT ON catalog_packages BEGIN
                INSERT INTO catalog_packages_fts(rowid, name, description)
                VALUES (new.rowid, new.name, new.description);
            END;
            "#,
        )?;

        Ok((temp_dir, conn))
    }

    #[test]
    fn select_installer_prefers_matching_arch() -> Result<()> {
        let installers = vec![
            sample_installer("", "portable"),
            sample_installer(current_arch_alias(), "msix"),
            sample_installer("fallback", "zip"),
        ];

        let selected = select_installer(&installers)?;

        assert_eq!(selected.arch, current_arch_alias());
        assert_eq!(selected.kind, "msix");

        Ok(())
    }

    #[test]
    fn select_installer_falls_back_to_blank_arch() -> Result<()> {
        let installers = vec![
            sample_installer("fallback", "zip"),
            sample_installer("", "portable"),
        ];

        let selected = select_installer(&installers)?;

        assert_eq!(selected.arch, "");
        assert_eq!(selected.kind, "portable");

        Ok(())
    }

    #[test]
    fn select_installer_errors_when_no_installers_exist() {
        let err = select_installer(&[]).expect_err("empty installer list should fail");

        assert!(
            err.to_string()
                .contains("catalog package has no installers")
        );
    }

    #[test]
    fn search_catalog_packages_returns_matches_from_catalog_db() -> Result<()> {
        let (_temp_dir, conn) = create_catalog_db()?;

        conn.execute(
            r#"
            INSERT INTO catalog_packages (
                id, name, version, source, description, homepage, license, publisher
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            "#,
            params![
                "Contoso.App",
                "Contoso Terminal",
                "1.2.3",
                "winget",
                Some("Terminal tools for Contoso users"),
                Option::<String>::None,
                Option::<String>::None,
                Some("Contoso Ltd."),
            ],
        )?;

        let matches = search_catalog_packages(&conn, "terminal")?;

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].id, "Contoso.App");
        assert_eq!(matches[0].name, "Contoso Terminal");
        assert_eq!(matches[0].source, "winget");

        Ok(())
    }
}
