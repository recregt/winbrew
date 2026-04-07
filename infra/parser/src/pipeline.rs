use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, Read};
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::ParserError;
use crate::metadata::{CatalogMetadata, write_metadata};
use crate::parser::{ParsedPackage, parse_package};
use crate::raw::RawFetchedPackage;
use crate::sqlite::CatalogWriter;
use crate::winget::read_winget_packages;

pub struct RunConfig {
    pub winget_db_path: PathBuf,
    pub output_db_path: PathBuf,
    pub metadata_path: PathBuf,
}

impl RunConfig {
    pub fn new(winget_db_path: PathBuf, output_db_path: PathBuf) -> Self {
        let metadata_path = output_db_path
            .parent()
            .map(|parent| parent.join("metadata.json"))
            .unwrap_or_else(|| PathBuf::from("metadata.json"));

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
        let key = package.package.source.as_str().to_string();
        *self.source_counts.entry(key).or_insert(0) += 1;
    }
}

fn stream_scoop_packages<R, F>(reader: R, mut on_package: F) -> Result<(), ParserError>
where
    R: BufRead,
    F: FnMut(ParsedPackage) -> Result<(), ParserError>,
{
    for (line_number, line_result) in reader.lines().enumerate() {
        let line = line_result?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let raw: RawFetchedPackage = match serde_json::from_str(trimmed) {
            Ok(raw) => raw,
            Err(source) => {
                return Err(ParserError::LineDecode {
                    line: line_number + 1,
                    source,
                });
            }
        };

        match parse_package(raw) {
            Ok(parsed) => on_package(parsed)?,
            Err(err) => eprintln!("skipping scoop package on line {}: {err}", line_number + 1),
        }
    }

    Ok(())
}

fn hash_file(path: &Path) -> Result<String, ParserError> {
    let mut file = fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];

    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    let digest = hasher.finalize();

    Ok(format!("sha256:{digest:x}"))
}
