use std::fs;
use std::io::BufRead;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};

use crate::error::ParserError;
use crate::metadata::{CatalogMetadata, write_metadata};
use crate::parser::{ParsedPackage, parse_package};
use crate::raw::RawFetchedPackage;
use crate::sqlite::write_catalog;
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
    let mut packages = read_scoop_packages(reader)?;
    let mut winget_packages = read_winget_packages(&config.winget_db_path)?;
    packages.append(&mut winget_packages);
    packages.sort_by(|left, right| left.package.id.cmp(&right.package.id));

    write_catalog(&config.output_db_path, &packages)?;
    let current_hash = hash_file(&config.output_db_path)?;
    let metadata = CatalogMetadata::build(&packages, current_hash);
    write_metadata(&config.metadata_path, &metadata)?;

    Ok(metadata)
}

fn read_scoop_packages<R: BufRead>(reader: R) -> Result<Vec<ParsedPackage>, ParserError> {
    let mut packages = Vec::new();

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
            Ok(parsed) => packages.push(parsed),
            Err(err) => eprintln!("skipping scoop package on line {}: {err}", line_number + 1),
        }
    }

    Ok(packages)
}

fn hash_file(path: &Path) -> Result<String, ParserError> {
    let bytes = fs::read(path)?;
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let digest = hasher.finalize();

    Ok(format!("sha256:{digest:x}"))
}
