use anyhow::{Context, Result, anyhow, bail};
use rusqlite::Connection;

use serde::Deserialize;
use serde_norway::Value;

use crate::manifest::{InstallerEntry, Manifest, ManifestInfo, Metadata, Package};
use crate::sources::{
    winget_manifest_format, winget_manifest_kind, winget_manifest_path_template,
    winget_registry_url,
};

pub(crate) fn manifest_url(conn: &Connection, name: &str, version: &str) -> Result<String> {
    let _ = conn;

    manifest_url_for(name, version)
}

pub(crate) fn manifest_url_for(name: &str, version: &str) -> Result<String> {
    let registry = winget_registry_url();
    let manifest_kind = winget_manifest_kind();
    let path_template = winget_manifest_path_template();
    let manifest_path = resolve_manifest_path(&path_template, name, version, &manifest_kind)?;

    Ok(format!(
        "{}/{}",
        registry.trim_end_matches('/'),
        manifest_path.trim_start_matches('/')
    ))
}

pub(crate) fn manifest_format(conn: &Connection) -> Result<String> {
    let _ = conn;

    Ok(winget_manifest_format())
}

pub(crate) fn parse_manifest(format: &str, content: &str) -> Result<Manifest> {
    match format.trim().to_ascii_lowercase().as_str() {
        "toml" | "winget_toml" | "custom_toml" => Manifest::parse_toml(content),
        "yaml" | "yml" | "winget_yaml" => parse_winget_yaml(content),
        other => bail!("unsupported manifest format: {other}"),
    }
}

fn resolve_manifest_path(
    template: &str,
    identifier: &str,
    version: &str,
    kind: &str,
) -> Result<String> {
    let segments: Vec<&str> = identifier
        .split('.')
        .filter(|segment| !segment.is_empty())
        .collect();
    if segments.len() < 2 {
        return Err(anyhow!(
            "winget package identifier must look like Publisher.Package, got: {identifier}"
        ));
    }

    let publisher = segments[0];
    let package = segments[1..].join("/");
    let partition = publisher
        .chars()
        .next()
        .ok_or_else(|| anyhow!("package identifier cannot be empty"))?
        .to_ascii_lowercase()
        .to_string();

    Ok(template
        .replace("${partition}", &partition)
        .replace("${publisher}", publisher)
        .replace("${package}", &package)
        .replace("${version}", version)
        .replace("${identifier}", identifier)
        .replace("${kind}", kind))
}

fn parse_winget_yaml(content: &str) -> Result<Manifest> {
    let raw: WingetManifest =
        serde_norway::from_str(content).context("failed to parse winget yaml")?;

    let manifest_type = raw
        .manifest_type
        .as_deref()
        .unwrap_or("singleton")
        .trim()
        .to_ascii_lowercase();

    if !matches!(manifest_type.as_str(), "installer" | "singleton") {
        bail!("SKIP_MANIFEST: unsupported winget manifest type: {manifest_type}");
    }

    let installers = normalize_installers(&raw)?;
    let dependencies = extract_dependencies(raw.dependencies.as_ref());

    let package = Package {
        name: raw.package_identifier.clone(),
        version: raw.package_version.clone(),
        package_name: raw.package_name,
        description: raw.short_description.or(raw.description),
        publisher: raw.publisher,
        homepage: raw.homepage,
        license: raw.license,
        moniker: raw.moniker,
        tags: raw.tags,
        dependencies,
    };

    if installers.is_empty() {
        return Err(anyhow!(
            "winget manifest must contain at least one installer"
        ));
    }

    Ok(Manifest {
        manifest: ManifestInfo {
            manifest_type: raw.manifest_type.unwrap_or_else(|| "installer".to_string()),
            manifest_version: raw.manifest_version.unwrap_or_else(|| "1.0.0".to_string()),
        },
        package,
        source: None,
        installers,
        metadata: raw.metadata.map(|metadata| Metadata {
            tags: metadata.tags,
            homepage: metadata.homepage,
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;

    fn fixture_path() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("winget")
            .join("windows-terminal.installer.yaml")
    }

    #[test]
    fn parses_winget_yaml_fixture_from_disk() {
        let content = fs::read_to_string(fixture_path()).expect("fixture file should exist");
        let manifest = parse_manifest("yaml", &content).expect("fixture should parse");

        assert_eq!(manifest.package.name, "Microsoft.WindowsTerminal");
        assert_eq!(
            manifest.package.package_name.as_deref(),
            Some("Windows Terminal")
        );
        assert_eq!(manifest.package.version, "1.21.2361.0");
        assert_eq!(
            manifest.package.publisher.as_deref(),
            Some("Microsoft Corporation")
        );
        assert_eq!(
            manifest.package.description.as_deref(),
            Some("Open source terminal application for developers.")
        );
        assert_eq!(manifest.installers.len(), 1);
        assert_eq!(manifest.installers[0].to_source().kind, "msix");
        assert_eq!(
            manifest.installers[0].display_name.as_deref(),
            Some("Windows Terminal")
        );
    }

    #[test]
    fn parses_winget_msi_fixture_from_disk() {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join("winget")
            .join("windows-terminal.msi.yaml");
        let content = fs::read_to_string(path).expect("fixture file should exist");
        let manifest = parse_manifest("yaml", &content).expect("fixture should parse");

        assert_eq!(manifest.package.name, "Microsoft.WindowsTerminal");
        assert_eq!(
            manifest.package.package_name.as_deref(),
            Some("Windows Terminal")
        );
        assert_eq!(manifest.package.version, "1.21.2361.0");
        assert_eq!(manifest.installers.len(), 1);
        assert_eq!(manifest.installers[0].installer_type, "msi");
        assert_eq!(
            manifest.installers[0].to_source().url,
            "https://example.invalid/WindowsTerminal.msi"
        );
        assert_eq!(
            manifest.installers[0].product_code.as_deref(),
            Some("{11111111-1111-1111-1111-111111111111}")
        );
    }
}

#[derive(Debug, Deserialize)]
struct WingetManifest {
    #[serde(rename = "ManifestType")]
    manifest_type: Option<String>,

    #[serde(rename = "ManifestVersion")]
    manifest_version: Option<String>,

    #[serde(rename = "PackageIdentifier")]
    package_identifier: String,

    #[serde(rename = "PackageVersion")]
    package_version: String,

    #[serde(rename = "Publisher")]
    publisher: Option<String>,

    #[serde(rename = "PackageName")]
    package_name: Option<String>,

    #[serde(rename = "ShortDescription")]
    short_description: Option<String>,

    #[serde(rename = "Description")]
    description: Option<String>,

    #[serde(rename = "Homepage")]
    homepage: Option<String>,

    #[serde(rename = "License")]
    license: Option<String>,

    #[serde(rename = "Moniker")]
    moniker: Option<String>,

    #[serde(rename = "Tags", default)]
    tags: Vec<String>,

    #[serde(rename = "Dependencies")]
    dependencies: Option<Value>,

    #[serde(rename = "Architecture")]
    architecture: Option<String>,

    #[serde(rename = "InstallerLocale")]
    installer_locale: Option<String>,

    #[serde(rename = "InstallerType")]
    installer_type: Option<String>,

    #[serde(rename = "InstallerUrl")]
    installer_url: Option<String>,

    #[serde(rename = "InstallerSha256")]
    installer_sha256: Option<String>,

    #[serde(rename = "Scope")]
    scope: Option<String>,

    #[serde(rename = "ProductCode")]
    product_code: Option<String>,

    #[serde(rename = "ReleaseDate")]
    release_date: Option<String>,

    #[serde(rename = "DisplayName")]
    display_name: Option<String>,

    #[serde(rename = "UpgradeBehavior")]
    upgrade_behavior: Option<String>,

    #[serde(rename = "Installers", default)]
    installers: Vec<WingetInstaller>,

    #[serde(rename = "Metadata")]
    metadata: Option<WingetMetadata>,
}

#[derive(Clone, Default, Debug, Deserialize)]
struct WingetInstaller {
    #[serde(rename = "Architecture")]
    architecture: Option<String>,

    #[serde(rename = "InstallerType")]
    installer_type: Option<String>,

    #[serde(rename = "InstallerUrl")]
    installer_url: Option<String>,

    #[serde(rename = "InstallerSha256")]
    installer_sha256: Option<String>,

    #[serde(rename = "InstallerLocale")]
    installer_locale: Option<String>,

    #[serde(rename = "Scope")]
    scope: Option<String>,

    #[serde(rename = "ProductCode")]
    product_code: Option<String>,

    #[serde(rename = "ReleaseDate")]
    release_date: Option<String>,

    #[serde(rename = "DisplayName")]
    display_name: Option<String>,

    #[serde(rename = "UpgradeBehavior")]
    upgrade_behavior: Option<String>,
}

#[derive(Debug, Deserialize)]
struct WingetMetadata {
    #[serde(rename = "Tags", default)]
    tags: Vec<String>,

    #[serde(rename = "Homepage")]
    homepage: Option<String>,
}

fn normalize_installers(raw: &WingetManifest) -> Result<Vec<InstallerEntry>> {
    let installer_defaults = InstallerDefaults {
        architecture: raw.architecture.clone(),
        installer_type: raw.installer_type.clone(),
        installer_url: raw.installer_url.clone(),
        installer_sha256: raw.installer_sha256.clone(),
        installer_locale: raw.installer_locale.clone(),
        scope: raw.scope.clone(),
        product_code: raw.product_code.clone(),
        release_date: raw.release_date.clone(),
        display_name: raw.display_name.clone(),
        upgrade_behavior: raw.upgrade_behavior.clone(),
    };

    let installer_nodes = if raw.installers.is_empty() {
        vec![WingetInstaller::default()]
    } else {
        raw.installers.clone()
    };

    installer_nodes
        .into_iter()
        .map(|installer| {
            let architecture = installer
                .architecture
                .or_else(|| installer_defaults.architecture.clone())
                .unwrap_or_else(|| "neutral".to_string());

            let installer_type = installer
                .installer_type
                .or_else(|| installer_defaults.installer_type.clone())
                .ok_or_else(|| anyhow!("winget installer entry is missing InstallerType"))?;

            let installer_url = installer
                .installer_url
                .or_else(|| installer_defaults.installer_url.clone())
                .ok_or_else(|| anyhow!("winget installer entry is missing InstallerUrl"))?;

            let installer_sha256 = installer
                .installer_sha256
                .or_else(|| installer_defaults.installer_sha256.clone())
                .ok_or_else(|| anyhow!("winget installer entry is missing InstallerSha256"))?;

            Ok(InstallerEntry {
                architecture,
                installer_type,
                installer_url,
                installer_sha256,
                installer_locale: installer
                    .installer_locale
                    .or_else(|| installer_defaults.installer_locale.clone()),
                scope: installer.scope.or_else(|| installer_defaults.scope.clone()),
                product_code: installer
                    .product_code
                    .or_else(|| installer_defaults.product_code.clone()),
                release_date: installer
                    .release_date
                    .or_else(|| installer_defaults.release_date.clone()),
                display_name: installer
                    .display_name
                    .or_else(|| installer_defaults.display_name.clone()),
                upgrade_behavior: installer
                    .upgrade_behavior
                    .or_else(|| installer_defaults.upgrade_behavior.clone()),
            })
        })
        .collect()
}

fn extract_dependencies(value: Option<&Value>) -> Vec<String> {
    let Some(value) = value else {
        return Vec::new();
    };

    let mut dependencies = Vec::new();

    if let Some(package_dependencies) = value
        .get("PackageDependencies")
        .and_then(|value| value.as_sequence())
    {
        for dependency in package_dependencies {
            if let Some(identifier) = dependency
                .get("PackageIdentifier")
                .and_then(|value| value.as_str())
            {
                dependencies.push(identifier.to_string());
                continue;
            }

            if let Some(identifier) = dependency.as_str() {
                dependencies.push(identifier.to_string());
            }
        }
    }

    dependencies
}

#[derive(Clone, Default)]
struct InstallerDefaults {
    architecture: Option<String>,
    installer_type: Option<String>,
    installer_url: Option<String>,
    installer_sha256: Option<String>,
    installer_locale: Option<String>,
    scope: Option<String>,
    product_code: Option<String>,
    release_date: Option<String>,
    display_name: Option<String>,
    upgrade_behavior: Option<String>,
}
