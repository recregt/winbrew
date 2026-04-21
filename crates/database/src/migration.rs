use anyhow::{Context, Result};
use rusqlite::Connection;

pub(crate) fn migrate(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS installed_packages (
            name         TEXT PRIMARY KEY,
            version      TEXT NOT NULL,
            kind         TEXT NOT NULL,
            deployment_kind TEXT NOT NULL DEFAULT 'installed',
            engine_kind  TEXT NOT NULL,
            engine_metadata TEXT,
            install_dir  TEXT NOT NULL,
            dependencies TEXT NOT NULL DEFAULT '[]',
            status       TEXT NOT NULL DEFAULT 'installing',
            installed_at TEXT NOT NULL
        );

        DROP TABLE IF EXISTS package_bin_lists;

        CREATE TABLE IF NOT EXISTS package_command_lists (
            package_name TEXT PRIMARY KEY REFERENCES installed_packages(name) ON DELETE CASCADE,
            commands_json TEXT NOT NULL DEFAULT '[]'
        );

        CREATE TABLE IF NOT EXISTS command_registry (
            command_name TEXT NOT NULL COLLATE NOCASE UNIQUE,
            package_name TEXT NOT NULL REFERENCES installed_packages(name) ON DELETE CASCADE
        );

        CREATE INDEX IF NOT EXISTS idx_command_registry_package_name
            ON command_registry(package_name);

        CREATE TABLE IF NOT EXISTS msi_receipts (
            package_name TEXT PRIMARY KEY REFERENCES installed_packages(name) ON DELETE CASCADE,
            product_code TEXT NOT NULL UNIQUE,
            upgrade_code TEXT,
            scope        TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS msi_files (
            package_name    TEXT NOT NULL REFERENCES installed_packages(name) ON DELETE CASCADE,
            path            TEXT NOT NULL,
            normalized_path TEXT NOT NULL,
            hash_algorithm  TEXT,
            hash_hex        TEXT,
            is_config_file  INTEGER NOT NULL DEFAULT 0,
            PRIMARY KEY (package_name, normalized_path)
        );

        CREATE INDEX IF NOT EXISTS idx_msi_files_normalized_path
            ON msi_files(normalized_path);

        CREATE TABLE IF NOT EXISTS msi_registry_entries (
            package_name        TEXT NOT NULL REFERENCES installed_packages(name) ON DELETE CASCADE,
            hive                TEXT NOT NULL,
            key_path            TEXT NOT NULL,
            normalized_key_path TEXT NOT NULL,
            value_name          TEXT NOT NULL,
            value_data          TEXT,
            previous_value      TEXT,
            PRIMARY KEY (package_name, hive, normalized_key_path, value_name)
        );

        CREATE INDEX IF NOT EXISTS idx_msi_registry_entries_normalized_key_path
            ON msi_registry_entries(normalized_key_path);

        CREATE TABLE IF NOT EXISTS msi_shortcuts (
            package_name           TEXT NOT NULL REFERENCES installed_packages(name) ON DELETE CASCADE,
            path                   TEXT NOT NULL,
            normalized_path        TEXT NOT NULL,
            target_path            TEXT,
            normalized_target_path TEXT,
            PRIMARY KEY (package_name, normalized_path)
        );

        CREATE INDEX IF NOT EXISTS idx_msi_shortcuts_normalized_path
            ON msi_shortcuts(normalized_path);

        CREATE INDEX IF NOT EXISTS idx_msi_shortcuts_normalized_target_path
            ON msi_shortcuts(normalized_target_path);

        CREATE TABLE IF NOT EXISTS msi_components (
            package_name    TEXT NOT NULL REFERENCES installed_packages(name) ON DELETE CASCADE,
            component_id    TEXT NOT NULL,
            path            TEXT,
            normalized_path TEXT,
            PRIMARY KEY (package_name, component_id)
        );

        CREATE INDEX IF NOT EXISTS idx_msi_components_component_id
            ON msi_components(component_id);

        CREATE INDEX IF NOT EXISTS idx_msi_components_normalized_path
            ON msi_components(normalized_path);
    ",
    )
    .context("migration failed")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::migrate;
    use rusqlite::{Connection, OptionalExtension};

    #[test]
    fn migrate_creates_msi_inventory_tables() {
        let conn = Connection::open_in_memory().expect("open in-memory database");

        migrate(&conn).expect("run migration");

        for table in [
            "installed_packages",
            "package_command_lists",
            "command_registry",
            "msi_receipts",
            "msi_files",
            "msi_registry_entries",
            "msi_shortcuts",
            "msi_components",
        ] {
            let exists = conn
                .query_row(
                    "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1",
                    [table],
                    |row| row.get::<_, i64>(0),
                )
                .expect("table lookup");

            assert_eq!(exists, 1, "expected table {table} to exist");
        }

        let index = "idx_command_registry_package_name";
        let exists = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'index' AND name = ?1",
                [index],
                |row| row.get::<_, i64>(0),
            )
            .expect("index lookup");

        assert_eq!(exists, 1, "expected index {index} to exist");
    }

    #[test]
    fn migrate_drops_obsolete_package_bin_lists_table() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        conn.execute_batch(
            "
            CREATE TABLE installed_packages (
                name TEXT PRIMARY KEY,
                version TEXT NOT NULL,
                kind TEXT NOT NULL,
                deployment_kind TEXT NOT NULL DEFAULT 'installed',
                engine_kind TEXT NOT NULL,
                engine_metadata TEXT,
                install_dir TEXT NOT NULL,
                dependencies TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL DEFAULT 'installing',
                installed_at TEXT NOT NULL
            );

            CREATE TABLE package_bin_lists (
                package_name TEXT PRIMARY KEY REFERENCES installed_packages(name) ON DELETE CASCADE,
                bin_json TEXT NOT NULL DEFAULT '[]'
            );
            ",
        )
        .expect("seed obsolete table");

        migrate(&conn).expect("run migration");

        let exists = conn
            .query_row(
                "SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'package_bin_lists'",
                [],
                |row| row.get::<_, i64>(0),
            )
            .optional()
            .expect("table lookup");

        assert!(
            exists.is_none(),
            "expected obsolete bin table to be dropped"
        );
    }
}
