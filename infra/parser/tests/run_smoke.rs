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

fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(name)
}

fn read_fixture(name: &str) -> Result<String, Box<dyn std::error::Error>> {
    Ok(fs::read_to_string(fixture_path(name))?)
}

#[derive(serde::Deserialize)]
struct FixtureEnvelope {
    payload: FixturePayload,
}

#[derive(serde::Deserialize)]
struct FixturePayload {
    installers: Vec<serde_json::Value>,
}

fn count_fixture_stats(input: &str) -> Result<(usize, usize), Box<dyn std::error::Error>> {
    let mut package_count = 0;
    let mut installer_count = 0;

    for line in input.lines().filter(|line| !line.trim().is_empty()) {
        let envelope: FixtureEnvelope = serde_json::from_str(line)?;
        package_count += 1;
        installer_count += envelope.payload.installers.len();
    }

    Ok((package_count, installer_count))
}

#[test]
fn run_builds_catalog_metadata_from_public_api() -> Result<(), Box<dyn std::error::Error>> {
    let root = unique_temp_dir("parser-integration");
    fs::create_dir_all(&root)?;
    let _guard = TempDirGuard(root.clone());

    let winget_jsonl_path = fixture_path("winget_packages.jsonl");
    let scoop_jsonl = read_fixture("scoop_packages.jsonl")?;
    let winget_jsonl = read_fixture("winget_packages.jsonl")?;

    let (scoop_package_count, scoop_installer_count) = count_fixture_stats(&scoop_jsonl)?;
    let (winget_package_count, winget_installer_count) = count_fixture_stats(&winget_jsonl)?;
    let expected_package_count = scoop_package_count + winget_package_count;
    let expected_installer_count = scoop_installer_count + winget_installer_count;

    let output_db_path = root.join("catalog.db");
    let metadata_path = root.join("metadata.json");

    let metadata = run(
        Cursor::new(scoop_jsonl.into_bytes()),
        RunConfig::new(winget_jsonl_path, output_db_path.clone())
            .with_metadata_path(metadata_path.clone()),
    )?;

    assert_eq!(metadata.schema_version, 1);
    assert_eq!(metadata.package_count, expected_package_count);
    assert_eq!(
        metadata.source_counts.get("scoop"),
        Some(&scoop_package_count)
    );
    assert_eq!(
        metadata.source_counts.get("winget"),
        Some(&winget_package_count)
    );
    assert!(metadata.current_hash.starts_with("sha256:"));
    assert!(metadata.previous_hash.is_empty());

    let metadata_text = fs::read_to_string(&metadata_path)?;
    let decoded: Value = serde_json::from_str(&metadata_text)?;
    assert_eq!(decoded["package_count"], expected_package_count);
    assert_eq!(decoded["source_counts"]["scoop"], scoop_package_count);
    assert_eq!(decoded["source_counts"]["winget"], winget_package_count);

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

    assert_eq!(package_count, i64::try_from(expected_package_count)?);
    assert_eq!(installer_count, i64::try_from(expected_installer_count)?);
    assert_eq!(schema_version, i64::from(CATALOG_DB_SCHEMA_VERSION));

    Ok(())
}
