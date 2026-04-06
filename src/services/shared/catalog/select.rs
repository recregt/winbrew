use thiserror::Error;

use crate::models::CatalogInstaller;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum InstallerSelectionError {
    #[error("catalog package has no installers")]
    NoInstallers,
}

pub fn select_installer(
    installers: &[CatalogInstaller],
) -> std::result::Result<CatalogInstaller, InstallerSelectionError> {
    let current_arch = current_arch_name();

    installers
        .iter()
        .find(|installer| installer.arch.eq_ignore_ascii_case(current_arch))
        .cloned()
        .or_else(|| {
            installers
                .iter()
                .find(|installer| installer.arch.trim().is_empty())
                .cloned()
        })
        .or_else(|| installers.first().cloned())
        .ok_or(InstallerSelectionError::NoInstallers)
}

fn current_arch_name() -> &'static str {
    match std::env::consts::ARCH {
        "x86_64" => "x64",
        "x86" => "x86",
        "aarch64" => "arm64",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    fn sample_installer(arch: &str, kind: &str) -> CatalogInstaller {
        CatalogInstaller {
            package_id: "Contoso.App".to_string(),
            url: "https://example.test/app.exe".to_string(),
            hash: "sha256:deadbeef".to_string(),
            arch: arch.to_string(),
            kind: kind.to_string(),
        }
    }

    fn current_arch_alias() -> &'static str {
        match std::env::consts::ARCH {
            "x86_64" => "x64",
            "x86" => "x86",
            "aarch64" => "arm64",
            other => other,
        }
    }

    #[test]
    fn select_installer_prefers_matching_arch() -> Result<()> {
        let installers = vec![
            sample_installer("", "portable"),
            sample_installer(current_arch_alias(), "msix"),
            sample_installer("fallback", "zip"),
        ];

        let selected = select_installer(&installers)?;

        assert_eq!(selected.arch, current_arch_alias());
        assert_eq!(selected.kind, "msix");

        Ok(())
    }

    #[test]
    fn select_installer_falls_back_to_blank_arch() -> Result<()> {
        let installers = vec![
            sample_installer("fallback", "zip"),
            sample_installer("", "portable"),
        ];

        let selected = select_installer(&installers)?;

        assert_eq!(selected.arch, "");
        assert_eq!(selected.kind, "portable");

        Ok(())
    }

    #[test]
    fn select_installer_errors_when_no_installers_exist() {
        let err = select_installer(&[]).expect_err("empty installer list should fail");

        assert!(
            err.to_string()
                .contains("catalog package has no installers")
        );
    }
}
