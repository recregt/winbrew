use anyhow::{Context, Result};
use rusqlite::{Connection, params};

use winbrew_models::{
    MsiComponentRecord, MsiFileRecord, MsiInventoryReceipt, MsiInventorySnapshot,
    MsiRegistryRecord, MsiShortcutRecord,
};

pub fn upsert_receipt(conn: &Connection, receipt: &MsiInventoryReceipt) -> Result<()> {
    conn.execute(
        "INSERT INTO msi_receipts (package_name, product_code, upgrade_code, scope)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(package_name) DO UPDATE SET
             product_code = excluded.product_code,
             upgrade_code = excluded.upgrade_code,
             scope = excluded.scope",
        params![
            receipt.package_name,
            receipt.product_code,
            receipt.upgrade_code,
            receipt.scope.to_string(),
        ],
    )
    .context("failed to upsert MSI receipt")?;

    Ok(())
}

pub fn replace_snapshot(conn: &mut Connection, snapshot: &MsiInventorySnapshot) -> Result<()> {
    let tx = conn
        .transaction()
        .context("failed to start MSI inventory transaction")?;

    upsert_receipt(&tx, &snapshot.receipt)?;

    tx.execute(
        "DELETE FROM msi_files WHERE package_name = ?1",
        params![snapshot.receipt.package_name],
    )
    .context("failed to clear MSI file inventory")?;
    tx.execute(
        "DELETE FROM msi_registry_entries WHERE package_name = ?1",
        params![snapshot.receipt.package_name],
    )
    .context("failed to clear MSI registry inventory")?;
    tx.execute(
        "DELETE FROM msi_shortcuts WHERE package_name = ?1",
        params![snapshot.receipt.package_name],
    )
    .context("failed to clear MSI shortcut inventory")?;
    tx.execute(
        "DELETE FROM msi_components WHERE package_name = ?1",
        params![snapshot.receipt.package_name],
    )
    .context("failed to clear MSI component inventory")?;

    insert_files(&tx, &snapshot.files)?;
    insert_registry_entries(&tx, &snapshot.registry_entries)?;
    insert_shortcuts(&tx, &snapshot.shortcuts)?;
    insert_components(&tx, &snapshot.components)?;

    tx.commit()
        .context("failed to commit MSI inventory snapshot")?;

    Ok(())
}

pub fn find_packages_by_normalized_path(
    conn: &Connection,
    normalized_path: &str,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT package_name
         FROM (
             SELECT package_name FROM msi_files WHERE normalized_path = ?1
             UNION
             SELECT package_name FROM msi_shortcuts
             WHERE normalized_path = ?1 OR normalized_target_path = ?1
             UNION
             SELECT package_name FROM msi_components WHERE normalized_path = ?1
         )
         ORDER BY package_name ASC",
    )?;

    stmt.query_map(params![normalized_path], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to read MSI path owners")
}

pub fn find_packages_by_normalized_registry_key_path(
    conn: &Connection,
    normalized_key_path: &str,
) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT package_name
         FROM msi_registry_entries
         WHERE normalized_key_path = ?1
         ORDER BY package_name ASC",
    )?;

    stmt.query_map(params![normalized_key_path], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to read MSI registry owners")
}

fn insert_files(conn: &Connection, files: &[MsiFileRecord]) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO msi_files
         (package_name, path, normalized_path, hash_algorithm, hash_hex, is_config_file)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
    )?;

    for file in files {
        stmt.execute(params![
            file.package_name,
            file.path,
            file.normalized_path,
            file.hash_algorithm.map(|algorithm| algorithm.to_string()),
            file.hash_hex,
            file.is_config_file,
        ])
        .context("failed to insert MSI file inventory row")?;
    }

    Ok(())
}

fn insert_registry_entries(conn: &Connection, entries: &[MsiRegistryRecord]) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO msi_registry_entries
         (package_name, hive, key_path, normalized_key_path, value_name, value_data, previous_value)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
    )?;

    for entry in entries {
        stmt.execute(params![
            entry.package_name,
            entry.hive,
            entry.key_path,
            entry.normalized_key_path,
            entry.value_name,
            entry.value_data,
            entry.previous_value,
        ])
        .context("failed to insert MSI registry inventory row")?;
    }

    Ok(())
}

fn insert_shortcuts(conn: &Connection, shortcuts: &[MsiShortcutRecord]) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO msi_shortcuts
         (package_name, path, normalized_path, target_path, normalized_target_path)
         VALUES (?1, ?2, ?3, ?4, ?5)",
    )?;

    for shortcut in shortcuts {
        stmt.execute(params![
            shortcut.package_name,
            shortcut.path,
            shortcut.normalized_path,
            shortcut.target_path,
            shortcut.normalized_target_path,
        ])
        .context("failed to insert MSI shortcut inventory row")?;
    }

    Ok(())
}

fn insert_components(conn: &Connection, components: &[MsiComponentRecord]) -> Result<()> {
    let mut stmt = conn.prepare(
        "INSERT INTO msi_components
         (package_name, component_id, path, normalized_path)
         VALUES (?1, ?2, ?3, ?4)",
    )?;

    for component in components {
        stmt.execute(params![
            component.package_name,
            component.component_id,
            component.path,
            component.normalized_path,
        ])
        .context("failed to insert MSI component inventory row")?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        find_packages_by_normalized_path, find_packages_by_normalized_registry_key_path,
        replace_snapshot,
    };
    use crate::database::{insert_package, migration};
    use rusqlite::Connection;
    use winbrew_models::{
        EngineKind, EngineMetadata, InstallScope, InstalledPackage, InstallerType,
        MsiComponentRecord, MsiFileRecord, MsiInventoryReceipt, MsiInventorySnapshot,
        MsiRegistryRecord, MsiShortcutRecord, PackageStatus,
    };

    fn sample_package(name: &str) -> InstalledPackage {
        InstalledPackage {
            name: name.to_string(),
            version: "1.0.0".to_string(),
            kind: InstallerType::Msi,
            engine_kind: EngineKind::Msi,
            engine_metadata: Some(EngineMetadata::Msi {
                product_code: "{11111111-1111-1111-1111-111111111111}".to_string(),
                upgrade_code: None,
                scope: InstallScope::Installed,
                registry_keys: Vec::new(),
                shortcuts: Vec::new(),
            }),
            install_dir: "C:/Tools/Demo".to_string(),
            dependencies: Vec::new(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-12T00:00:00Z".to_string(),
        }
    }

    fn sample_snapshot(name: &str) -> MsiInventorySnapshot {
        MsiInventorySnapshot {
            receipt: MsiInventoryReceipt {
                package_name: name.to_string(),
                product_code: "{11111111-1111-1111-1111-111111111111}".to_string(),
                upgrade_code: Some("{22222222-2222-2222-2222-222222222222}".to_string()),
                scope: InstallScope::Installed,
            },
            files: vec![MsiFileRecord {
                package_name: name.to_string(),
                path: "C:/Tools/Demo/bin/demo.exe".to_string(),
                normalized_path: "c:/tools/demo/bin/demo.exe".to_string(),
                hash_algorithm: Some(winbrew_models::HashAlgorithm::Sha256),
                hash_hex: Some(
                    "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
                ),
                is_config_file: false,
            }],
            registry_entries: vec![MsiRegistryRecord {
                package_name: name.to_string(),
                hive: "HKLM".to_string(),
                key_path: "Software\\Demo".to_string(),
                normalized_key_path: "software\\demo".to_string(),
                value_name: "InstallPath".to_string(),
                value_data: Some("C:/Tools/Demo".to_string()),
                previous_value: None,
            }],
            shortcuts: vec![MsiShortcutRecord {
                package_name: name.to_string(),
                path: "C:/Users/Public/Desktop/Demo.lnk".to_string(),
                normalized_path: "c:/users/public/desktop/demo.lnk".to_string(),
                target_path: Some("C:/Tools/Demo/bin/demo.exe".to_string()),
                normalized_target_path: Some("c:/tools/demo/bin/demo.exe".to_string()),
            }],
            components: vec![MsiComponentRecord {
                package_name: name.to_string(),
                component_id: "COMPONENT-DEMO".to_string(),
                path: Some("C:/Tools/Demo/bin/demo.exe".to_string()),
                normalized_path: Some("c:/tools/demo/bin/demo.exe".to_string()),
            }],
        }
    }

    #[test]
    fn replace_snapshot_persists_inventory_and_reverse_lookup() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        migration::migrate(&conn).expect("run migration");

        let package_name = "demo";
        insert_package(&conn, &sample_package(package_name)).expect("insert package");

        let mut conn = conn;
        replace_snapshot(&mut conn, &sample_snapshot(package_name)).expect("replace snapshot");

        let file_owners = find_packages_by_normalized_path(&conn, "c:/tools/demo/bin/demo.exe")
            .expect("lookup file owners");
        assert_eq!(file_owners, vec![package_name.to_string()]);

        let registry_owners =
            find_packages_by_normalized_registry_key_path(&conn, "software\\demo")
                .expect("lookup registry owners");
        assert_eq!(registry_owners, vec![package_name.to_string()]);
    }

    #[test]
    fn replace_snapshot_overwrites_previous_rows() {
        let conn = Connection::open_in_memory().expect("open in-memory database");
        migration::migrate(&conn).expect("run migration");

        let package_name = "demo";
        insert_package(&conn, &sample_package(package_name)).expect("insert package");

        let mut conn = conn;
        replace_snapshot(&mut conn, &sample_snapshot(package_name))
            .expect("insert initial snapshot");

        let mut updated = sample_snapshot(package_name);
        updated.files = vec![MsiFileRecord {
            package_name: package_name.to_string(),
            path: "C:/Tools/Demo/bin/demo2.exe".to_string(),
            normalized_path: "c:/tools/demo/bin/demo2.exe".to_string(),
            hash_algorithm: Some(winbrew_models::HashAlgorithm::Sha256),
            hash_hex: Some(
                "fedcba9876543210fedcba9876543210fedcba9876543210fedcba9876543210".to_string(),
            ),
            is_config_file: true,
        }];
        updated.shortcuts = vec![MsiShortcutRecord {
            package_name: package_name.to_string(),
            path: "C:/Users/Public/Desktop/Demo2.lnk".to_string(),
            normalized_path: "c:/users/public/desktop/demo2.lnk".to_string(),
            target_path: Some("C:/Tools/Demo/bin/demo2.exe".to_string()),
            normalized_target_path: Some("c:/tools/demo/bin/demo2.exe".to_string()),
        }];
        updated.components = vec![MsiComponentRecord {
            package_name: package_name.to_string(),
            component_id: "COMPONENT-DEMO-2".to_string(),
            path: Some("C:/Tools/Demo/bin/demo2.exe".to_string()),
            normalized_path: Some("c:/tools/demo/bin/demo2.exe".to_string()),
        }];

        replace_snapshot(&mut conn, &updated).expect("replace snapshot");

        let old_owners = find_packages_by_normalized_path(&conn, "c:/tools/demo/bin/demo.exe")
            .expect("lookup old owners");
        assert!(old_owners.is_empty());

        let new_owners = find_packages_by_normalized_path(&conn, "c:/tools/demo/bin/demo2.exe")
            .expect("lookup new owners");
        assert_eq!(new_owners, vec![package_name.to_string()]);
    }
}
