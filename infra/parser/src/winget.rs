use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use crate::error::ParserError;
use crate::parser::{ParsedPackage, parse_package};
use crate::raw::RawFetchedPackage;

const WINGET_STREAM_SCHEMA_VERSION: u32 = 1;
const WINGET_STREAM_SOURCE: &str = "winget";
const WINGET_STREAM_KIND: &str = "package";

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct WingetStreamEnvelope {
    schema_version: u32,
    source: String,
    kind: String,
    payload: RawFetchedPackage,
}

impl WingetStreamEnvelope {
    fn validate(&self) -> Result<(), ParserError> {
        if self.schema_version != WINGET_STREAM_SCHEMA_VERSION {
            return Err(ParserError::Contract(format!(
                "unsupported winget stream schema version: expected {WINGET_STREAM_SCHEMA_VERSION}, got {}",
                self.schema_version
            )));
        }

        if self.source != WINGET_STREAM_SOURCE {
            return Err(ParserError::Contract(format!(
                "expected {WINGET_STREAM_SOURCE}, got {}",
                self.source
            )));
        }

        if self.kind != WINGET_STREAM_KIND {
            return Err(ParserError::Contract(format!(
                "expected {WINGET_STREAM_KIND}, got {}",
                self.kind
            )));
        }

        Ok(())
    }
}

pub fn read_winget_packages<F>(path: &Path, mut on_package: F) -> Result<(), ParserError>
where
    F: FnMut(ParsedPackage) -> Result<(), ParserError>,
{
    let file = File::open(path)?;
    let mut reader = BufReader::new(file);
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

        let envelope: WingetStreamEnvelope = match serde_json::from_slice(&line) {
            Ok(raw) => raw,
            Err(source) => {
                return Err(ParserError::LineDecode {
                    line: line_number,
                    source,
                });
            }
        };

        envelope.validate()?;

        match parse_package(envelope.payload) {
            Ok(parsed) => on_package(parsed)?,
            Err(err) => eprintln!("skipping winget package on line {}: {err}", line_number),
        }
    }

    Ok(())
}
