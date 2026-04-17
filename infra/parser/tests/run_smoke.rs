use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use std::process;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::Connection;
use serde_json::Value;
use winbrew_infra_parser::{RunConfig, run};
use winbrew_models::catalog::metadata::CATALOG_DB_SCHEMA_VERSION;

struct TempDirGuard(PathBuf);

impl Drop for TempDirGuard {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn unique_temp_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!("winbrew-{name}-{}-{stamp}", process::id()))
}

fn write_winget_jsonl(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let envelope = serde_json::json!({
        "schema_version": 1,
        "source": "winget",
        "kind": "package",
        "payload": {
            "id": "winget/Contoso.App",
            "name": "Contoso App",
            "version": "2.0.0",
            "description": null,
            "homepage": null,
            "license": null,
            "publisher": "Contoso Ltd.",
            "locale": "en-US",
            "moniker": "contoso",
            "tags": ["utility"],
            "bin": null,
            "installers": []
        }
    });

    fs::write(path, format!("{}\n", serde_json::to_string(&envelope)?))?;

    Ok(())
}

#[test]
fn run_builds_catalog_metadata_from_public_api() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_dir("parser-integration");
    fs::create_dir_all(&root)?;
    let _guard = TempDirGuard(root.clone());

    let winget_jsonl_path = root.join("winget.jsonl");
    write_winget_jsonl(&winget_jsonl_path)?;

    let output_db_path = root.join("catalog.db");
    let metadata_path = root.join("metadata.json");

    let scoop_jsonl = r#"
{"schema_version":1,"source":"scoop","kind":"package","payload":{"id":"scoop/main/example","name":"Example Tool","version":"1.2.3","description":"Example package","homepage":"https://example.invalid","license":"MIT","publisher":"Example Corp","installers":[{"url":"https://example.invalid/example.zip","hash":"abcd","arch":"x64","type":"portable"}]}}
"#;

    let metadata = run(
        Cursor::new(scoop_jsonl.as_bytes().to_vec()),
        RunConfig::new(winget_jsonl_path.clone(), output_db_path.clone())
            .with_metadata_path(metadata_path.clone()),
    )?;

    assert_eq!(metadata.schema_version, 1);
    assert_eq!(metadata.package_count, 2);
    assert_eq!(metadata.source_counts.get("scoop"), Some(&1));
    assert_eq!(metadata.source_counts.get("winget"), Some(&1));
    assert!(metadata.current_hash.starts_with("sha256:"));
    assert!(metadata.previous_hash.is_empty());

    let metadata_text = fs::read_to_string(&metadata_path)?;
    let decoded: Value = serde_json::from_str(&metadata_text)?;
    assert_eq!(decoded["package_count"], 2);
    assert_eq!(decoded["source_counts"]["scoop"], 1);
    assert_eq!(decoded["source_counts"]["winget"], 1);

    let connection = Connection::open(&output_db_path)?;
    let package_count: i64 =
        connection.query_row("SELECT COUNT(*) FROM catalog_packages", [], |row| {
            row.get(0)
        })?;
    let installer_count: i64 =
        connection.query_row("SELECT COUNT(*) FROM catalog_installers", [], |row| {
            row.get(0)
        })?;
    let schema_version: i64 = connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    assert_eq!(package_count, 2);
    assert_eq!(installer_count, 1);
    assert_eq!(schema_version, i64::from(CATALOG_DB_SCHEMA_VERSION));

    Ok(())
}
