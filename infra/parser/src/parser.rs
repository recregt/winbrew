use winbrew_models::{
    Architecture, CatalogInstaller, CatalogPackage, InstallerType, PackageSource, Version,
};

use crate::error::ParserError;
use crate::raw::{RawFetchedInstaller, RawFetchedPackage, ScoopStreamEnvelope};

#[derive(Debug, Clone)]
pub struct ParsedPackage {
    pub package: CatalogPackage,
    pub installers: Vec<CatalogInstaller>,
    pub raw_json: String,
}

pub fn parse_package(raw: RawFetchedPackage) -> Result<ParsedPackage, ParserError> {
    let raw_json = serde_json::to_string(&raw)?;

    let package = CatalogPackage {
        id: raw.id.clone().into(),
        name: raw.name,
        version: Version::parse_lossy(&raw.version)?,
        source: PackageSource::from_catalog_id(&raw.id),
        description: raw.description,
        homepage: raw.homepage,
        license: raw.license,
        publisher: raw.publisher,
    };
    package.validate()?;

    let installers = raw
        .installers
        .into_iter()
        .map(|installer| parse_installer(&raw.id, installer))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(ParsedPackage {
        package,
        installers,
        raw_json,
    })
}

pub fn parse_packages(
    raw_packages: Vec<RawFetchedPackage>,
) -> Result<Vec<ParsedPackage>, ParserError> {
    raw_packages.into_iter().map(parse_package).collect()
}

pub fn parse_packages_json(input: &str) -> Result<Vec<ParsedPackage>, ParserError> {
    let envelopes: Vec<ScoopStreamEnvelope> = serde_json::from_str(input)?;
    let raw_packages = envelopes
        .into_iter()
        .map(validate_envelope)
        .collect::<Result<Vec<_>, _>>()?;
    parse_packages(raw_packages)
}

fn parse_installer(
    package_id: &str,
    raw: RawFetchedInstaller,
) -> Result<CatalogInstaller, ParserError> {
    let installer = CatalogInstaller {
        package_id: package_id.into(),
        url: raw.url,
        hash: raw.hash,
        arch: raw.arch.parse::<Architecture>()?,
        kind: raw.kind.parse::<InstallerType>()?,
    };
    installer.validate()?;
    Ok(installer)
}

fn validate_envelope(envelope: ScoopStreamEnvelope) -> Result<RawFetchedPackage, ParserError> {
    envelope
        .validate()
        .map_err(|err| ParserError::Contract(err.to_string()))?;
    Ok(envelope.payload)
}

#[cfg(test)]
mod tests {
    use super::{parse_package, parse_packages_json};
    use crate::error::ParserError;
    use crate::raw::{RawFetchedInstaller, RawFetchedPackage};
    use winbrew_models::{Architecture, InstallerType, PackageSource};

    #[test]
    fn parses_fetched_package_into_shared_models() {
        let parsed = parse_package(RawFetchedPackage {
            id: "winget/Contoso.App".to_string(),
            name: "Contoso App".to_string(),
            version: "1.2.3".to_string(),
            description: Some("Example".to_string()),
            homepage: None,
            license: None,
            publisher: Some("Contoso Ltd.".to_string()),
            installers: vec![RawFetchedInstaller {
                url: "https://example.invalid/app.exe".to_string(),
                hash: "".to_string(),
                arch: "x64".to_string(),
                kind: "portable".to_string(),
            }],
        })
        .expect("package should parse");

        assert_eq!(parsed.package.source, PackageSource::Winget);
        assert_eq!(parsed.installers[0].arch, Architecture::X64);
        assert_eq!(parsed.installers[0].kind, InstallerType::Portable);
        assert!(parsed.raw_json.contains("Contoso.App"));
    }

    #[test]
    fn parses_fetched_package_with_loose_version() {
        let parsed = parse_package(RawFetchedPackage {
            id: "winget/Wez.WezTerm".to_string(),
            name: "WezTerm".to_string(),
            version: "v2026.03.17".to_string(),
            description: None,
            homepage: None,
            license: None,
            publisher: Some("Wez Furlong".to_string()),
            installers: vec![RawFetchedInstaller {
                url: "https://example.invalid/wezterm.zip".to_string(),
                hash: String::new(),
                arch: "x64".to_string(),
                kind: "portable".to_string(),
            }],
        })
        .expect("package should parse");

        assert_eq!(parsed.package.version.to_string(), "2026.3.17");
    }

    #[test]
    fn parses_package_list_from_json() {
        let json = r#"
        [
            {
                "schema_version": 1,
                "source": "scoop",
                "kind": "package",
                "payload": {
                    "id": "scoop/main/Contoso.Tool",
                    "name": "Contoso Tool",
                    "version": "2.0.0",
                    "description": null,
                    "homepage": null,
                    "license": null,
                    "publisher": null,
                    "installers": []
                }
            }
        ]
        "#;

        let parsed = parse_packages_json(json).expect("json should parse");

        assert_eq!(parsed.len(), 1);
        assert_eq!(parsed[0].package.source, PackageSource::Scoop);
        assert_eq!(parsed[0].package.version.to_string(), "2.0.0");
    }

    #[test]
    fn rejects_unknown_envelope_version() {
        let json = r#"
        [
            {
                "schema_version": 2,
                "source": "scoop",
                "kind": "package",
                "payload": {
                    "id": "scoop/main/Contoso.Tool",
                    "name": "Contoso Tool",
                    "version": "2.0.0",
                    "description": null,
                    "homepage": null,
                    "license": null,
                    "publisher": null,
                    "installers": []
                }
            }
        ]
        "#;

        let err = parse_packages_json(json).expect_err("version mismatch should fail");
        assert!(
            err.to_string()
                .contains("unsupported scoop stream schema version")
        );
    }

    #[test]
    fn rejects_unknown_envelope_field() {
        let json = r#"
        [
            {
                "schema_version": 1,
                "source": "scoop",
                "kind": "package",
                "unexpected": true,
                "payload": {
                    "id": "scoop/main/Contoso.Tool",
                    "name": "Contoso Tool",
                    "version": "2.0.0",
                    "description": null,
                    "homepage": null,
                    "license": null,
                    "publisher": null,
                    "installers": []
                }
            }
        ]
        "#;

        let err = parse_packages_json(json).expect_err("unknown field should fail");
        assert!(matches!(err, ParserError::Decode(_)));
    }
}
