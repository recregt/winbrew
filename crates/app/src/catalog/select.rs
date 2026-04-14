//! Installer selection policy for catalog packages.
//!
//! Catalog packages can publish multiple installers for different architectures
//! or package kinds. This module centralizes the ranking rules so callers do not
//! need to replicate architecture fallback logic in command code.
//!
//! The selection order is intentionally simple and predictable:
//!
//! - prefer an installer that exactly matches the current architecture
//! - fall back to `Architecture::Any` when no exact match exists
//! - fall back to the first available installer if nothing else matches

use tracing::debug;
use winbrew_models::domains::catalog::CatalogInstaller;
use winbrew_models::domains::install::Architecture;

/// Select the best installer for the current architecture.
///
/// The helper returns `None` when the catalog package has no installers. The
/// caller is responsible for translating that into a user-facing install error.
#[must_use = "selected installer should be used or explicitly discarded"]
pub(crate) fn select_installer(installers: &[CatalogInstaller]) -> Option<CatalogInstaller> {
    if installers.is_empty() {
        return None;
    }

    let current_arch = Architecture::current();
    debug!(
        installer_count = installers.len(),
        current_arch = ?current_arch,
        "selecting best installer"
    );

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    fn sample_installer(
        arch: Architecture,
        kind: winbrew_models::domains::install::InstallerType,
    ) -> CatalogInstaller {
        CatalogInstaller::test_builder("Contoso.App".into(), "https://example.test/app.exe")
            .with_arch(arch)
            .with_kind(kind)
    }

    fn non_current_arches() -> (Architecture, Architecture) {
        match Architecture::current() {
            Architecture::X64 => (Architecture::X86, Architecture::Arm64),
            Architecture::X86 => (Architecture::X64, Architecture::Arm64),
            Architecture::Arm64 => (Architecture::X64, Architecture::X86),
            Architecture::Any => (Architecture::X64, Architecture::X86),
        }
    }

    #[test]
    fn select_installer_prefers_matching_arch() -> Result<()> {
        let installers = vec![
            sample_installer(
                Architecture::Any,
                winbrew_models::domains::install::InstallerType::Portable,
            ),
            sample_installer(
                Architecture::current(),
                winbrew_models::domains::install::InstallerType::Msix,
            ),
            sample_installer(
                Architecture::X64,
                winbrew_models::domains::install::InstallerType::Zip,
            ),
        ];

        let selected = select_installer(&installers).expect("installer should exist");

        assert_eq!(selected.arch, Architecture::current());
        assert_eq!(
            selected.kind,
            winbrew_models::domains::install::InstallerType::Msix
        );

        Ok(())
    }

    #[test]
    fn select_installer_returns_single_installer() -> Result<()> {
        let installers = vec![sample_installer(
            Architecture::Arm64,
            winbrew_models::domains::install::InstallerType::Exe,
        )];

        let selected = select_installer(&installers).expect("installer should exist");

        assert_eq!(selected.arch, Architecture::Arm64);
        assert_eq!(
            selected.kind,
            winbrew_models::domains::install::InstallerType::Exe
        );

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
            sample_installer(
                non_matching_arch,
                winbrew_models::domains::install::InstallerType::Zip,
            ),
            sample_installer(
                Architecture::Any,
                winbrew_models::domains::install::InstallerType::Portable,
            ),
        ];

        let selected = select_installer(&installers).expect("installer should exist");

        assert_eq!(selected.arch, Architecture::Any);
        assert_eq!(
            selected.kind,
            winbrew_models::domains::install::InstallerType::Portable
        );

        Ok(())
    }

    #[test]
    fn select_installer_falls_back_to_first_available_installer() -> Result<()> {
        let (first_arch, second_arch) = non_current_arches();

        let installers = vec![
            sample_installer(
                first_arch,
                winbrew_models::domains::install::InstallerType::Exe,
            ),
            sample_installer(
                second_arch,
                winbrew_models::domains::install::InstallerType::Portable,
            ),
        ];

        let selected = select_installer(&installers).expect("installer should exist");

        assert_eq!(selected.arch, first_arch);
        assert_eq!(
            selected.kind,
            winbrew_models::domains::install::InstallerType::Exe
        );

        Ok(())
    }

    #[test]
    fn select_installer_returns_none_when_no_installers_exist() {
        assert!(select_installer(&[]).is_none());
    }
}
