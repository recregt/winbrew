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
    pub winget_db_path: PathBuf,
    pub output_db_path: PathBuf,
    pub metadata_path: PathBuf,
}

impl RunConfig {
    pub fn new(winget_db_path: PathBuf, output_db_path: PathBuf) -> Self {
        let metadata_path = output_db_path.parent().map_or_else(
            || PathBuf::from("metadata.json"),
            |parent| parent.join("metadata.json"),
        );

        Self {
            winget_db_path,
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
    let mut writer = CatalogWriter::open(&config.output_db_path)?;
    let mut stats = CatalogStats::default();

    stream_scoop_packages(reader, |package| {
        stats.record(&package);
        writer.write_package(&package)
    })?;

    read_winget_packages(&config.winget_db_path, |package| {
        stats.record(&package);
        writer.write_package(&package)
    })?;

    writer.finish()?;

    let current_hash = hash_file(&config.output_db_path)?;
    let metadata =
        CatalogMetadata::build_from_counts(stats.package_count, stats.source_counts, current_hash);
    write_metadata(&config.metadata_path, &metadata)?;

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
            Err(err) => eprintln!("skipping scoop package on line {}: {err}", line_number),
        }
    }

    Ok(())
}

fn hash_file(path: &Path) -> Result<String, ParserError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 65536];

    loop {
        let read = file.read(&mut buffer)?;
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

    fn unique_temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("winbrew-{name}-{}-{stamp}", process::id()))
    }

    fn create_winget_db(path: &PathBuf) -> Result<(), Box<dyn std::error::Error>> {
        let connection = Connection::open(path)?;
        connection.execute_batch(
            r#"
            CREATE TABLE ids (id TEXT NOT NULL);
            CREATE TABLE names (name TEXT NOT NULL);
            CREATE TABLE versions (version TEXT NOT NULL);
            CREATE TABLE manifest (id INTEGER NOT NULL, name INTEGER NOT NULL, version INTEGER NOT NULL);
            CREATE TABLE norm_publishers (norm_publisher TEXT NOT NULL);
            CREATE TABLE norm_publishers_map (manifest INTEGER NOT NULL, norm_publisher INTEGER NOT NULL);
            "#,
        )?;

        connection.execute("INSERT INTO ids(id) VALUES (?1)", ["Contoso.App"])?;
        let id_rowid = connection.last_insert_rowid();
        connection.execute("INSERT INTO names(name) VALUES (?1)", ["Contoso App"])?;
        let name_rowid = connection.last_insert_rowid();
        connection.execute("INSERT INTO versions(version) VALUES (?1)", ["2.0.0"])?;
        let version_rowid = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO manifest(id, name, version) VALUES (?1, ?2, ?3)",
            [id_rowid, name_rowid, version_rowid],
        )?;
        let manifest_rowid = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO norm_publishers(norm_publisher) VALUES (?1)",
            ["Contoso Ltd."],
        )?;
        let publisher_rowid = connection.last_insert_rowid();
        connection.execute(
            "INSERT INTO norm_publishers_map(manifest, norm_publisher) VALUES (?1, ?2)",
            [manifest_rowid, publisher_rowid],
        )?;

        Ok(())
    }

    #[test]
    fn run_builds_catalog_metadata_from_streamed_and_staged_inputs()
    -> Result<(), Box<dyn std::error::Error>> {
        let root = unique_temp_dir("parser-e2e");
        fs::create_dir_all(&root)?;

        let winget_db_path = root.join("winget.db");
        create_winget_db(&winget_db_path)?;

        let output_db_path = root.join("catalog.db");
        let metadata_path = root.join("metadata.json");

        let scoop_jsonl = r#"
    {"schema_version":1,"source":"scoop","kind":"package","payload":{"id":"scoop/main/example","name":"Example Tool","version":"1.2.3","description":"Example package","homepage":"https://example.invalid","license":"MIT","publisher":"Example Corp","installers":[{"url":"https://example.invalid/example.zip","hash":"abcd","arch":"x64","type":"portable"}]}}
    "#;

        let metadata = run(
            Cursor::new(scoop_jsonl.as_bytes().to_vec()),
            RunConfig::new(winget_db_path.clone(), output_db_path.clone())
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
        assert_eq!(schema_version, 1);

        Ok(())
    }
}
