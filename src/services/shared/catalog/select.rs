use thiserror::Error;

use crate::models::{Architecture, CatalogInstaller};

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum InstallerSelectionError {
    #[error("catalog package has no installers")]
    NoInstallers,
}

pub fn select_installer(
    installers: &[CatalogInstaller],
) -> std::result::Result<CatalogInstaller, InstallerSelectionError> {
    let current_arch = Architecture::current();

    installers
        .iter()
        .find(|installer| installer.arch == current_arch)
        .cloned()
        .or_else(|| {
            installers
                .iter()
                .find(|installer| installer.arch == Architecture::Any)
                .cloned()
        })
        .or_else(|| installers.first().cloned())
        .ok_or(InstallerSelectionError::NoInstallers)
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    fn sample_installer(
        arch: Architecture,
        kind: crate::models::InstallerType,
    ) -> CatalogInstaller {
        CatalogInstaller {
            package_id: "Contoso.App".to_string(),
            url: "https://example.test/app.exe".to_string(),
            hash: "sha256:deadbeef".to_string(),
            arch,
            kind,
        }
    }

    #[test]
    fn select_installer_prefers_matching_arch() -> Result<()> {
        let installers = vec![
            sample_installer(Architecture::Any, crate::models::InstallerType::Portable),
            sample_installer(Architecture::current(), crate::models::InstallerType::Msix),
            sample_installer(Architecture::X64, crate::models::InstallerType::Zip),
        ];

        let selected = select_installer(&installers)?;

        assert_eq!(selected.arch, Architecture::current());
        assert_eq!(selected.kind, crate::models::InstallerType::Msix);

        Ok(())
    }

    #[test]
    fn select_installer_falls_back_to_blank_arch() -> Result<()> {
        let non_matching_arch = match Architecture::current() {
            Architecture::X64 => Architecture::X86,
            Architecture::X86 => Architecture::X64,
            Architecture::Arm64 => Architecture::X64,
            Architecture::Any => Architecture::X64,
        };

        let installers = vec![
            sample_installer(non_matching_arch, crate::models::InstallerType::Zip),
            sample_installer(Architecture::Any, crate::models::InstallerType::Portable),
        ];

        let selected = select_installer(&installers)?;

        assert_eq!(selected.arch, Architecture::Any);
        assert_eq!(selected.kind, crate::models::InstallerType::Portable);

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
