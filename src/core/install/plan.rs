use anyhow::{Result, bail};
use std::path::{Component, Path, PathBuf};

use crate::core::paths;
use crate::database;
use crate::manifest::{Manifest, Source};

#[derive(Debug, Clone)]
pub struct InstallPlan {
    pub name: String,
    pub package_version: String,
    pub source: Source,
    pub cache_file: PathBuf,
    pub install_dir: PathBuf,
    pub backup_dir: PathBuf,
    pub product_code: Option<String>,
    pub dependencies: Vec<String>,
}

pub fn build_plan(name: &str, manifest: &Manifest) -> Result<InstallPlan> {
    validate_package_name(name)?;

    let install_root = install_root();
    let source = manifest
        .selected_source()
        .unwrap_or_else(|| manifest.source.clone());
    let package_version = manifest.package.version.clone();
    let ext = detect_ext(&source.url);
    let cache_file = paths::cache_file(name, &package_version, &ext);
    let install_dir = paths::package_dir_at(&install_root, name);

    Ok(InstallPlan {
        name: name.to_string(),
        package_version,
        source,
        cache_file,
        install_dir: install_dir.clone(),
        backup_dir: backup_dir_for(&install_dir),
        product_code: manifest
            .preferred_installer()
            .and_then(|entry| entry.product_code.clone()),
        dependencies: manifest.package.dependencies.clone(),
    })
}

pub fn backup_dir_for(install_dir: &Path) -> PathBuf {
    install_dir.with_extension("backup")
}

pub fn install_root() -> PathBuf {
    let (root, _) = database::get_effective_value("paths.root")
        .unwrap_or_else(|_| (database::Config::current().paths.root, "file"));

    PathBuf::from(root)
}

pub fn detect_ext(url: &str) -> String {
    let url_path = url.split(['?', '#']).next().unwrap_or(url);

    Path::new(url_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("exe")
        .to_ascii_lowercase()
}

pub fn source_file_name(url: &str) -> Option<String> {
    let url_path = url.split(['?', '#']).next().unwrap_or(url);

    Path::new(url_path)
        .file_name()
        .and_then(|name| name.to_str())
        .map(|name| name.to_string())
}

fn validate_package_name(name: &str) -> Result<()> {
    let trimmed = name.trim();

    if trimmed.is_empty() {
        bail!("invalid package name: empty value");
    }

    if trimmed
        .chars()
        .any(|ch| matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'))
    {
        bail!("invalid package name: forbidden Windows filename characters detected");
    }

    if !Path::new(trimmed)
        .components()
        .all(|component| matches!(component, Component::Normal(_)))
    {
        bail!("invalid package name: traversal characters detected");
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::{InstallerEntry, Manifest, ManifestInfo, Package, Source};

    fn manifest_with_installer(
        installer_type: &str,
        url: &str,
        product_code: Option<&str>,
    ) -> Manifest {
        Manifest {
            manifest: ManifestInfo::default(),
            package: Package {
                name: "Microsoft.WindowsTerminal".to_string(),
                version: "1.21.2361.0".to_string(),
                package_name: Some("Windows Terminal".to_string()),
                description: Some("Terminal".to_string()),
                publisher: Some("Microsoft Corporation".to_string()),
                homepage: None,
                license: None,
                moniker: None,
                tags: vec![],
                dependencies: vec!["Microsoft.VCLibs".to_string()],
            },
            source: Source {
                url: url.to_string(),
                checksum: "abc123".to_string(),
                kind: installer_type.to_string(),
            },
            installers: vec![InstallerEntry {
                architecture: "x64".to_string(),
                installer_type: installer_type.to_string(),
                installer_url: url.to_string(),
                installer_sha256: "abc123".to_string(),
                installer_locale: None,
                scope: None,
                product_code: product_code.map(ToOwned::to_owned),
                release_date: None,
                display_name: None,
                upgrade_behavior: None,
            }],
            metadata: None,
        }
    }

    #[test]
    fn detect_ext_ignores_query_string() {
        assert_eq!(
            detect_ext("https://example.invalid/app.msi?download=1"),
            "msi"
        );
    }

    #[test]
    fn source_file_name_ignores_fragment_and_query() {
        assert_eq!(
            source_file_name("https://example.invalid/app.msixbundle?download=1#anchor"),
            Some("app.msixbundle".to_string())
        );
    }

    #[test]
    fn build_plan_uses_preferred_installer_and_install_root() {
        let manifest = manifest_with_installer(
            "msi",
            "https://example.invalid/WindowsTerminal.msi",
            Some("{11111111-1111-1111-1111-111111111111}"),
        );

        let plan = build_plan("Microsoft.WindowsTerminal", &manifest).expect("plan should build");

        assert_eq!(plan.name, "Microsoft.WindowsTerminal");
        assert_eq!(plan.package_version, "1.21.2361.0");
        assert_eq!(plan.source.kind, "msi");
        assert_eq!(
            plan.product_code.as_deref(),
            Some("{11111111-1111-1111-1111-111111111111}")
        );
        assert!(
            plan.install_dir
                .ends_with(r"packages\Microsoft.WindowsTerminal")
        );
        assert!(
            plan.cache_file
                .ends_with(r"winbrew\data\cache\Microsoft.WindowsTerminal-1.21.2361.0.msi")
        );
        assert_eq!(plan.backup_dir.parent(), plan.install_dir.parent());
        assert_eq!(
            plan.backup_dir.extension().and_then(|ext| ext.to_str()),
            Some("backup")
        );
        assert_eq!(plan.dependencies, vec!["Microsoft.VCLibs".to_string()]);
    }

    #[test]
    fn detect_ext_normalizes_uppercase_extensions() {
        assert_eq!(
            detect_ext("https://example.invalid/app.MSI?download=1"),
            "msi"
        );
    }

    #[test]
    fn build_plan_rejects_traversal_package_names() {
        let manifest = manifest_with_installer(
            "msi",
            "https://example.invalid/WindowsTerminal.msi",
            Some("{11111111-1111-1111-1111-111111111111}"),
        );

        let err = build_plan("..", &manifest).expect_err("plan should fail");

        assert!(
            err.to_string()
                .contains("invalid package name: traversal characters detected")
        );
    }

    #[test]
    fn build_plan_rejects_windows_forbidden_filename_characters() {
        let manifest = manifest_with_installer(
            "msi",
            "https://example.invalid/WindowsTerminal.msi",
            Some("{11111111-1111-1111-1111-111111111111}"),
        );

        let err = build_plan("Microsoft:WindowsTerminal", &manifest).expect_err("plan should fail");

        assert!(
            err.to_string()
                .contains("invalid package name: forbidden Windows filename characters detected")
        );
    }
}
