use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::{BufRead, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::ParserError;
use crate::metadata::{CatalogMetadata, write_metadata};
use crate::parser::{ParsedPackage, parse_package};
use crate::raw::ScoopStreamEnvelope;
use crate::sqlite::CatalogWriter;
use crate::winget::read_winget_packages;

pub struct RunConfig {
    pub winget_jsonl_path: PathBuf,
    pub output_db_path: PathBuf,
    pub metadata_path: PathBuf,
}

impl RunConfig {
    pub fn new(winget_jsonl_path: PathBuf, output_db_path: PathBuf) -> Self {
        let metadata_path = output_db_path.parent().map_or_else(
            || PathBuf::from("metadata.json"),
            |parent| parent.join("metadata.json"),
        );

        Self {
            winget_jsonl_path,
            output_db_path,
            metadata_path,
        }
    }

    pub fn with_metadata_path(mut self, metadata_path: PathBuf) -> Self {
        self.metadata_path = metadata_path;
        self
    }
}

pub fn run<R: BufRead>(reader: R, config: RunConfig) -> Result<CatalogMetadata, ParserError> {
    eprintln!(
        "[parser] starting catalog materialization winget_jsonl={} out={} metadata={}",
        config.winget_jsonl_path.display(),
        config.output_db_path.display(),
        config.metadata_path.display(),
    );

    let mut writer = CatalogWriter::open(&config.output_db_path)?;
    let mut stats = CatalogStats::default();

    eprintln!("[parser] ingesting scoop JSONL from stdin");
    stream_scoop_packages(reader, |package| {
        stats.record(&package);
        writer.write_package(&package)
    })?;
    eprintln!("[parser] scoop ingestion complete");

    eprintln!(
        "[parser] ingesting winget JSONL from {}",
        config.winget_jsonl_path.display()
    );
    read_winget_packages(&config.winget_jsonl_path, |package| {
        stats.record(&package);
        writer.write_package(&package)
    })?;
    eprintln!("[parser] winget ingestion complete");

    eprintln!(
        "[parser] finalizing catalog database at {}",
        config.output_db_path.display()
    );
    writer.finish()?;

    let current_hash = hash_file(&config.output_db_path)?;
    let metadata =
        CatalogMetadata::build_from_counts(stats.package_count, stats.source_counts, current_hash);
    eprintln!(
        "[parser] writing catalog metadata to {}",
        config.metadata_path.display()
    );
    write_metadata(&config.metadata_path, &metadata)?;
    eprintln!(
        "[parser] catalog materialization complete packages={} sources={:?}",
        metadata.package_count, metadata.source_counts
    );

    Ok(metadata)
}

#[derive(Default)]
struct CatalogStats {
    package_count: usize,
    source_counts: BTreeMap<String, usize>,
}

impl CatalogStats {
    fn record(&mut self, package: &ParsedPackage) {
        self.package_count += 1;
        let source = package.package.source.as_str();

        if let Some(count) = self.source_counts.get_mut(source) {
            *count += 1;
        } else {
            self.source_counts.insert(source.to_string(), 1);
        }
    }
}

fn stream_scoop_packages<R, F>(mut reader: R, mut on_package: F) -> Result<(), ParserError>
where
    R: BufRead,
    F: FnMut(ParsedPackage) -> Result<(), ParserError>,
{
    let mut line = Vec::new();
    let mut line_number = 0;

    loop {
        line.clear();
        let bytes_read = reader.read_until(b'\n', &mut line)?;
        if bytes_read == 0 {
            break;
        }

        line_number += 1;
        if line.iter().all(|byte| byte.is_ascii_whitespace()) {
            continue;
        }

        let envelope: ScoopStreamEnvelope = match serde_json::from_slice(&line) {
            Ok(raw) => raw,
            Err(source) => {
                return Err(ParserError::LineDecode {
                    line: line_number,
                    source,
                });
            }
        };

        if let Err(err) = envelope.validate() {
            return Err(ParserError::Contract(format!(
                "failed to decode scoop envelope on line {line_number}: {err}"
            )));
        }

        match parse_package(envelope.payload) {
            Ok(parsed) => on_package(parsed)?,
            Err(err) => eprintln!(
                "[parser] skipping scoop package on line {}: {err}",
                line_number
            ),
        }
    }

    Ok(())
}

fn hash_file(path: &Path) -> Result<String, ParserError> {
    let mut file = fs::File::open(path).map_err(|source| {
        ParserError::io_with_context(source, format!("opening {}", path.display()))
    })?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];

    loop {
        let read = file.read(&mut buffer).map_err(|source| {
            ParserError::io_with_context(source, format!("hashing {}", path.display()))
        })?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let digest = hasher.finalize();
    let mut digest_hex = String::with_capacity(digest.len() * 2);

    for byte in digest {
        write!(&mut digest_hex, "{byte:02x}").expect("write digest hex");
    }

    Ok(format!("sha256:{digest_hex}"))
}

#[cfg(test)]
mod tests {
    use super::RunConfig;
    use super::run;
    use rusqlite::Connection;
    use serde_json::Value;
    use std::fs;
    use std::io::Cursor;
    use std::path::PathBuf;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};
    use winbrew_models::catalog::metadata::CATALOG_DB_SCHEMA_VERSION;

    fn unique_temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("winbrew-{name}-{}-{stamp}", process::id()))
    }

    fn create_winget_jsonl(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
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
    fn run_builds_catalog_metadata_from_streamed_and_staged_inputs()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = unique_temp_dir("parser-e2e");
        fs::create_dir_all(&root)?;

        let winget_jsonl_path = root.join("winget.jsonl");
        create_winget_jsonl(&winget_jsonl_path)?;

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
        let schema_version: i64 =
            connection.query_row("PRAGMA user_version", [], |row| row.get(0))?;
        assert_eq!(package_count, 2);
        assert_eq!(installer_count, 1);
        assert_eq!(schema_version, i64::from(CATALOG_DB_SCHEMA_VERSION));

        Ok(())
    }

    fn winget_envelope(id: &str, version: &str, installer_hash: Option<&str>) -> Value {
        let installers = match installer_hash {
            Some(hash) => serde_json::json!([{
                "url": "https://example.invalid/app.exe",
                "hash": hash,
                "arch": "x64",
                "type": "exe"
            }]),
            None => serde_json::json!([]),
        };

        serde_json::json!({
            "schema_version": 1,
            "source": "winget",
            "kind": "package",
            "payload": {
                "id": id,
                "name": "Test App",
                "version": version,
                "description": null,
                "homepage": null,
                "license": null,
                "publisher": "Test Publisher",
                "locale": "en-US",
                "moniker": "testapp",
                "tags": ["utility"],
                "bin": null,
                "installers": installers
            }
        })
    }

    fn package_rowid(connection: &Connection, id: &str) -> rusqlite::Result<i64> {
        connection.query_row(
            "SELECT rowid FROM catalog_packages WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
    }

    fn package_version(connection: &Connection, id: &str) -> rusqlite::Result<String> {
        connection.query_row(
            "SELECT version FROM catalog_packages WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
    }

    fn package_exists(connection: &Connection, id: &str) -> rusqlite::Result<bool> {
        let count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM catalog_packages WHERE id = ?1",
            [id],
            |row| row.get(0),
        )?;
        Ok(count > 0)
    }

    fn installer_id(connection: &Connection, package_id: &str, hash: &str) -> rusqlite::Result<i64> {
        connection.query_row(
            "SELECT id FROM catalog_installers WHERE package_id = ?1 AND hash = ?2",
            rusqlite::params![package_id, hash],
            |row| row.get(0),
        )
    }

    /// A parser run over an existing catalog must upsert in place rather than
    /// rebuild from scratch: unchanged rows keep their `rowid`/`id` (the
    /// property patch generation depends on), updated rows are overwritten
    /// without gaining a new identity, packages missing from the run are
    /// pruned, and new packages get fresh rows.
    #[test]
    fn run_upserts_into_existing_catalog_and_preserves_row_identity()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = unique_temp_dir("parser-stability");
        fs::create_dir_all(&root)?;

        let winget_jsonl_path = root.join("winget.jsonl");
        let output_db_path = root.join("catalog.db");
        let metadata_path = root.join("metadata.json");

        // Run 1: package A (with an installer whose hash will later change)
        // and package B, sourced from scoop.
        let run1_envelopes = [
            winget_envelope("winget/Retained.App", "2.0.0", Some("sha256:aaaa")),
        ];
        fs::write(
            &winget_jsonl_path,
            run1_envelopes
                .iter()
                .map(|envelope| serde_json::to_string(envelope).unwrap())
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        )?;

        let scoop_jsonl_run1 = r#"{"schema_version":1,"source":"scoop","kind":"package","payload":{"id":"scoop/main/removed","name":"Removed Tool","version":"1.0.0","description":null,"homepage":null,"license":null,"publisher":null,"installers":[]}}
"#;

        run(
            Cursor::new(scoop_jsonl_run1.as_bytes().to_vec()),
            RunConfig::new(winget_jsonl_path.clone(), output_db_path.clone())
                .with_metadata_path(metadata_path.clone()),
        )?;

        let (retained_rowid, removed_installer_id) = {
            let connection = Connection::open(&output_db_path)?;
            assert!(package_exists(&connection, "scoop/main/removed")?);
            (
                package_rowid(&connection, "winget/Retained.App")?,
                installer_id(&connection, "winget/Retained.App", "sha256:aaaa")?,
            )
        };

        // Run 2, same output path: A is updated (version bump, installer
        // hash rotated), B disappears (no longer crawled), C is new.
        let run2_envelopes = [
            winget_envelope("winget/Retained.App", "3.0.0", Some("sha256:bbbb")),
            winget_envelope("winget/New.App", "1.0.0", None),
        ];
        fs::write(
            &winget_jsonl_path,
            run2_envelopes
                .iter()
                .map(|envelope| serde_json::to_string(envelope).unwrap())
                .collect::<Vec<_>>()
                .join("\n")
                + "\n",
        )?;

        let metadata2 = run(
            Cursor::new(Vec::new()),
            RunConfig::new(winget_jsonl_path.clone(), output_db_path.clone())
                .with_metadata_path(metadata_path.clone()),
        )?;

        assert_eq!(metadata2.package_count, 2);

        let connection = Connection::open(&output_db_path)?;

        // Retained package keeps its rowid across runs and reflects the update.
        assert_eq!(
            package_rowid(&connection, "winget/Retained.App")?,
            retained_rowid
        );
        assert_eq!(
            package_version(&connection, "winget/Retained.App")?,
            "3.0.0"
        );

        // The stale installer row (old hash) is gone; the new hash got a
        // fresh id, since its canonical identity genuinely changed.
        let stale_installer_count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM catalog_installers WHERE id = ?1",
            [removed_installer_id],
            |row| row.get(0),
        )?;
        assert_eq!(stale_installer_count, 0);
        assert!(installer_id(&connection, "winget/Retained.App", "sha256:bbbb").is_ok());

        // Package missing from this run is pruned outright.
        assert!(!package_exists(&connection, "scoop/main/removed")?);

        // A genuinely new package gets a row.
        assert!(package_exists(&connection, "winget/New.App")?);

        Ok(())
    }

    /// Re-writing a package with unchanged installers must not touch the
    /// installer row's id, since patch generation keys off it.
    #[test]
    fn run_preserves_installer_id_when_installer_is_unchanged()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = unique_temp_dir("parser-installer-stability");
        fs::create_dir_all(&root)?;

        let winget_jsonl_path = root.join("winget.jsonl");
        let output_db_path = root.join("catalog.db");
        let metadata_path = root.join("metadata.json");

        for version in ["1.0.0", "1.0.1"] {
            let envelope = winget_envelope("winget/Stable.App", version, Some("sha256:cccc"));
            fs::write(&winget_jsonl_path, format!("{}\n", serde_json::to_string(&envelope)?))?;

            run(
                Cursor::new(Vec::new()),
                RunConfig::new(winget_jsonl_path.clone(), output_db_path.clone())
                    .with_metadata_path(metadata_path.clone()),
            )?;
        }

        let connection = Connection::open(&output_db_path)?;
        let installer_count: i64 = connection.query_row(
            "SELECT COUNT(*) FROM catalog_installers WHERE package_id = 'winget/Stable.App'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(
            installer_count, 1,
            "unchanged installer must be updated in place, not duplicated or replaced"
        );

        Ok(())
    }
}
