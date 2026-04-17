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
            Err(err) => eprintln!(
                "[parser] skipping winget package on line {}: {err}",
                line_number
            ),
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::read_winget_packages;
    use std::fs;
    use std::path::PathBuf;
    use std::process;
    use std::time::{SystemTime, UNIX_EPOCH};
    use winbrew_models::catalog::CatalogInstallerType;
    use winbrew_models::install::installer::{Architecture, InstallerType};
    use winbrew_models::package::PackageSource;
    use winbrew_models::shared::HashAlgorithm;

    fn unique_temp_file(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        std::env::temp_dir().join(format!("winbrew-{name}-{}-{stamp}.jsonl", process::id()))
    }

    #[test]
    fn reads_merged_winget_envelope() -> Result<(), Box<dyn std::error::Error>> {
        let path = unique_temp_file("winget-reader");
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let envelope = serde_json::json!({
            "schema_version": 1,
            "source": "winget",
            "kind": "package",
            "payload": {
                "id": "winget/Contoso.App",
                "name": "Contoso App",
                "version": "2.3.4",
                "description": "Contoso app",
                "homepage": "https://contoso.example",
                "license": "MIT",
                "publisher": "Contoso Ltd.",
                "locale": "en-US",
                "moniker": "contoso",
                "tags": ["utility"],
                "bin": null,
                "installers": [
                    {
                        "url": "https://example.invalid/app.exe",
                        "hash": "sha256:abcd",
                        "arch": "x64",
                        "type": "exe",
                        "NestedInstallerType": "portable",
                        "installer_switches": "/S",
                        "scope": "machine"
                    }
                ]
            }
        });
        fs::write(&path, format!("{}\n", serde_json::to_string(&envelope)?))?;

        let mut parsed = Vec::new();
        read_winget_packages(&path, |package| {
            parsed.push(package);
            Ok(())
        })?;

        assert_eq!(parsed.len(), 1);
        let package = &parsed[0];
        assert_eq!(package.package.source, PackageSource::Winget);
        assert_eq!(package.package.source_id, "Contoso.App");
        assert_eq!(package.package.name, "Contoso App");
        assert_eq!(package.package.publisher.as_deref(), Some("Contoso Ltd."));
        assert_eq!(package.installers.len(), 1);
        assert_eq!(package.installers[0].arch, Architecture::X64);
        assert_eq!(package.installers[0].kind, InstallerType::Exe);
        assert_eq!(
            package.installers[0].nested_kind,
            Some(InstallerType::Portable)
        );
        assert_eq!(
            package.installers[0].installer_switches.as_deref(),
            Some("/S")
        );
        assert_eq!(package.installers[0].scope.as_deref(), Some("machine"));
        assert_eq!(package.installers[0].hash_algorithm, HashAlgorithm::Sha256);
        assert_eq!(
            package.installers[0].installer_type,
            CatalogInstallerType::Exe
        );

        Ok(())
    }
}
