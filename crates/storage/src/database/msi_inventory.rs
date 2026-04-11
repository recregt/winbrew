use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};

use winbrew_models::{
    HashAlgorithm, InstallScope, MsiComponentRecord, MsiFileRecord, MsiInventoryReceipt,
    MsiInventorySnapshot, MsiRegistryRecord, MsiShortcutRecord,
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

pub fn apply_snapshot(conn: &Connection, snapshot: &MsiInventorySnapshot) -> Result<()> {
    upsert_receipt(conn, &snapshot.receipt)?;

    conn.execute(
        "DELETE FROM msi_files WHERE package_name = ?1",
        params![snapshot.receipt.package_name],
    )
    .context("failed to clear MSI file inventory")?;
    conn.execute(
        "DELETE FROM msi_registry_entries WHERE package_name = ?1",
        params![snapshot.receipt.package_name],
    )
    .context("failed to clear MSI registry inventory")?;
    conn.execute(
        "DELETE FROM msi_shortcuts WHERE package_name = ?1",
        params![snapshot.receipt.package_name],
    )
    .context("failed to clear MSI shortcut inventory")?;
    conn.execute(
        "DELETE FROM msi_components WHERE package_name = ?1",
        params![snapshot.receipt.package_name],
    )
    .context("failed to clear MSI component inventory")?;

    insert_files(conn, &snapshot.files)?;
    insert_registry_entries(conn, &snapshot.registry_entries)?;
    insert_shortcuts(conn, &snapshot.shortcuts)?;
    insert_components(conn, &snapshot.components)?;

    Ok(())
}

pub fn replace_snapshot(conn: &mut Connection, snapshot: &MsiInventorySnapshot) -> Result<()> {
    let tx = conn
        .transaction()
        .context("failed to start MSI inventory transaction")?;

    apply_snapshot(&tx, snapshot)?;

    tx.commit()
        .context("failed to commit MSI inventory snapshot")?;

    Ok(())
}

pub fn get_snapshot(conn: &Connection, package_name: &str) -> Result<Option<MsiInventorySnapshot>> {
    let Some(receipt) = load_receipt(conn, package_name)? else {
        return Ok(None);
    };

    Ok(Some(MsiInventorySnapshot {
        receipt,
        files: load_files(conn, package_name)?,
        registry_entries: load_registry_entries(conn, package_name)?,
        shortcuts: load_shortcuts(conn, package_name)?,
        components: load_components(conn, package_name)?,
    }))
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

fn load_receipt(conn: &Connection, package_name: &str) -> Result<Option<MsiInventoryReceipt>> {
    let mut stmt = conn.prepare(
        "SELECT package_name, product_code, upgrade_code, scope
         FROM msi_receipts
         WHERE package_name = ?1",
    )?;

    let receipt = stmt
        .query_row(params![package_name], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, Option<String>>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .optional()
        .context("failed to read MSI receipt")?;

    let Some((package_name, product_code, upgrade_code, scope_raw)) = receipt else {
        return Ok(None);
    };

    let scope = scope_raw
        .parse::<InstallScope>()
        .with_context(|| format!("failed to parse MSI receipt scope for {package_name}"))?;

    Ok(Some(MsiInventoryReceipt {
        package_name,
        product_code,
        upgrade_code,
        scope,
    }))
}

fn load_files(conn: &Connection, package_name: &str) -> Result<Vec<MsiFileRecord>> {
    let mut stmt = conn.prepare(
        "SELECT package_name, path, normalized_path, hash_algorithm, hash_hex, is_config_file
         FROM msi_files
         WHERE package_name = ?1
         ORDER BY normalized_path ASC",
    )?;

    let rows = stmt
        .query_map(params![package_name], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, Option<String>>(3)?,
                row.get::<_, Option<String>>(4)?,
                row.get::<_, bool>(5)?,
            ))
        })?
        .collect::<std::result::Result<Vec<_>, _>>()?;

    rows.into_iter()
        .map(
            |(
                package_name,
                path,
                normalized_path,
                hash_algorithm_raw,
                hash_hex,
                is_config_file,
            )| {
                Ok(MsiFileRecord {
                    package_name,
                    path,
                    normalized_path,
                    hash_algorithm: hash_algorithm_raw
                        .map(|value| value.parse::<HashAlgorithm>())
                        .transpose()
                        .context("failed to parse MSI file hash algorithm")?,
                    hash_hex,
                    is_config_file,
                })
            },
        )
        .collect()
}

fn load_registry_entries(conn: &Connection, package_name: &str) -> Result<Vec<MsiRegistryRecord>> {
    let mut stmt = conn.prepare(
        "SELECT package_name, hive, key_path, normalized_key_path, value_name, value_data, previous_value
         FROM msi_registry_entries
         WHERE package_name = ?1
         ORDER BY hive ASC, normalized_key_path ASC, value_name ASC",
    )?;

    stmt.query_map(params![package_name], |row| {
        Ok(MsiRegistryRecord {
            package_name: row.get(0)?,
            hive: row.get(1)?,
            key_path: row.get(2)?,
            normalized_key_path: row.get(3)?,
            value_name: row.get(4)?,
            value_data: row.get(5)?,
            previous_value: row.get(6)?,
        })
    })?
    .collect::<std::result::Result<Vec<_>, _>>()
    .context("failed to read MSI registry entries")
}

fn load_shortcuts(conn: &Connection, package_name: &str) -> Result<Vec<MsiShortcutRecord>> {
    let mut stmt = conn.prepare(
        "SELECT package_name, path, normalized_path, target_path, normalized_target_path
         FROM msi_shortcuts
         WHERE package_name = ?1
         ORDER BY normalized_path ASC",
    )?;

    stmt.query_map(params![package_name], |row| {
        Ok(MsiShortcutRecord {
            package_name: row.get(0)?,
            path: row.get(1)?,
            normalized_path: row.get(2)?,
            target_path: row.get(3)?,
            normalized_target_path: row.get(4)?,
        })
    })?
    .collect::<std::result::Result<Vec<_>, _>>()
    .context("failed to read MSI shortcuts")
}

fn load_components(conn: &Connection, package_name: &str) -> Result<Vec<MsiComponentRecord>> {
    let mut stmt = conn.prepare(
        "SELECT package_name, component_id, path, normalized_path
         FROM msi_components
         WHERE package_name = ?1
         ORDER BY component_id ASC",
    )?;

    stmt.query_map(params![package_name], |row| {
        Ok(MsiComponentRecord {
            package_name: row.get(0)?,
            component_id: row.get(1)?,
            path: row.get(2)?,
            normalized_path: row.get(3)?,
        })
    })?
    .collect::<std::result::Result<Vec<_>, _>>()
    .context("failed to read MSI components")
}

#[cfg(test)]
mod tests {
    use super::{
        find_packages_by_normalized_path, find_packages_by_normalized_registry_key_path,
        get_snapshot, replace_snapshot,
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

        let snapshot = get_snapshot(&conn, package_name)
            .expect("load snapshot")
            .expect("snapshot present");
        assert_eq!(snapshot, sample_snapshot(package_name));

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
