use anyhow::{Result, bail};

use crate::manifest::Manifest;

pub trait ManifestParser {
    fn parse(&self, content: &str) -> Result<Manifest>;
}

pub struct TomlManifestParser;

impl ManifestParser for TomlManifestParser {
    fn parse(&self, content: &str) -> Result<Manifest> {
        Manifest::parse_toml(content)
    }
}

pub fn parse_manifest(format: &str, content: &str) -> Result<Manifest> {
    match format.trim().to_ascii_lowercase().as_str() {
        "toml" | "winget_toml" | "custom_toml" => TomlManifestParser.parse(content),
        other => bail!("unsupported manifest format: {other}"),
    }
}