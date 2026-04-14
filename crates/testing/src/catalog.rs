use crate::core::hash::hash_algorithm;
use crate::models::shared::hash::HashAlgorithm as CatalogHashAlgorithm;
use anyhow::Result;
use rusqlite::{Connection, params};

pub fn catalog_package_id(package_name: &str) -> String {
    format!("winget/{}", package_name.replace(' ', "."))
}

pub fn seed_catalog_package(
    conn: &Connection,
    package_name: &str,
    description: &str,
    installer_url: &str,
    hash: &str,
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
            id, name, version, source, namespace, source_id, description, homepage, license, publisher
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
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
            "zip",
            Option::<String>::None,
            "",
            "zip",
        ],
    )?;

    Ok(())
}
