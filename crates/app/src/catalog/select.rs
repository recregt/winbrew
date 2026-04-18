//! Host-aware installer selection policy for catalog packages.
//!
//! Catalog packages can publish multiple installers for different architectures
//! or platform families. This module centralizes the ranking rules so callers
//! do not need to replicate host-profile fallback logic in command code.
//!
//! The selection order is intentionally simple and predictable:
//!
//! - ignore installers whose platform metadata does not match the host family
//! - prefer an installer that exactly matches the host architecture
//! - fall back to `Architecture::Any` when no exact match exists
//! - fall back to the first compatible installer if nothing else matches

use crate::models::domains::catalog::CatalogInstaller;
use crate::models::domains::install::Architecture;
use crate::windows::HostKind;
use thiserror::Error;
use tracing::debug;

/// Raised when catalog installers cannot be matched to the current host.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallerSelectionError {
    /// The catalog package has no installers at all.
    #[error("catalog package has no installers")]
    NoInstallers,
    /// The package has installers, but none match this host's platform family.
    #[error("no installer matches this host ({host_kind} {host_architecture})")]
    NoCompatibleInstaller {
        host_kind: HostKind,
        host_architecture: Architecture,
    },
}

/// Select the best installer for the current architecture.
///
/// The helper returns a structured error when the catalog package has no
/// installers or when none of the installers are compatible with this host.
pub(crate) fn select_installer(
    installers: &[CatalogInstaller],
    host_kind: HostKind,
    host_architecture: Architecture,
) -> Result<CatalogInstaller, InstallerSelectionError> {
    if installers.is_empty() {
        return Err(InstallerSelectionError::NoInstallers);
    }

    let compatible_installers: Vec<&CatalogInstaller> = installers
        .iter()
        .filter(|installer| platform_matches_host(installer.platform.as_deref(), host_kind))
        .collect();

    if compatible_installers.is_empty() {
        return Err(InstallerSelectionError::NoCompatibleInstaller {
            host_kind,
            host_architecture,
        });
    }

    debug!(
        installer_count = installers.len(),
        compatible_count = compatible_installers.len(),
        host_kind = %host_kind,
        host_architecture = %host_architecture,
        "selecting best installer"
    );

    let selected = compatible_installers
        .iter()
        .find(|installer| installer.arch == host_architecture)
        .copied()
        .or_else(|| {
            compatible_installers
                .iter()
                .find(|installer| installer.arch == Architecture::Any)
                .copied()
        })
        .or_else(|| compatible_installers.first().copied())
        .expect("compatible installers should not be empty");

    Ok(selected.clone())
}

fn platform_matches_host(platform: Option<&str>, host_kind: HostKind) -> bool {
    let Some(platform) = platform else {
        return true;
    };

    let Ok(platform_values) = serde_json::from_str::<Vec<String>>(platform) else {
        return false;
    };

    let accepted_platforms = host_kind.platform_tags();

    platform_values
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .any(|value| accepted_platforms.iter().any(|accepted| value == *accepted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    fn sample_installer(
        arch: Architecture,
        kind: winbrew_models::domains::install::InstallerType,
        platform: Option<&str>,
    ) -> CatalogInstaller {
        let mut installer =
            CatalogInstaller::test_builder("Contoso.App".into(), "https://example.test/app.exe")
                .with_arch(arch)
                .with_kind(kind);

        installer.platform = platform.map(str::to_string);
        installer
    }

    fn normal_host() -> HostKind {
        HostKind::Normal
    }

    fn server_host() -> HostKind {
        HostKind::Server
    }

    #[test]
    fn select_installer_prefers_matching_arch_for_normal_hosts() -> Result<()> {
        let installers = vec![
            sample_installer(
                Architecture::Any,
                winbrew_models::domains::install::InstallerType::Portable,
                Some("[\"Windows.Desktop\"]"),
            ),
            sample_installer(
                Architecture::X64,
                winbrew_models::domains::install::InstallerType::Msix,
                Some("[\"Windows.Desktop\"]"),
            ),
            sample_installer(
                Architecture::X86,
                winbrew_models::domains::install::InstallerType::Zip,
                Some("[\"Windows.Desktop\"]"),
            ),
        ];

        let selected = select_installer(&installers, normal_host(), Architecture::X64)
            .expect("installer should exist");

        assert_eq!(selected.arch, Architecture::X64);
        assert_eq!(
            selected.kind,
            winbrew_models::domains::install::InstallerType::Msix
        );

        Ok(())
    }

    #[test]
    fn select_installer_prefers_server_platform_on_server_hosts() -> Result<()> {
        let installers = vec![
            sample_installer(
                Architecture::X64,
                winbrew_models::domains::install::InstallerType::Exe,
                Some("[\"Windows.Desktop\"]"),
            ),
            sample_installer(
                Architecture::X64,
                winbrew_models::domains::install::InstallerType::Portable,
                Some("[\"Windows.Server\"]"),
            ),
            sample_installer(
                Architecture::Any,
                winbrew_models::domains::install::InstallerType::Zip,
                Some("[\"Windows.Server\"]"),
            ),
        ];

        let selected = select_installer(&installers, server_host(), Architecture::X64)
            .expect("installer should exist");

        assert_eq!(selected.arch, Architecture::X64);
        assert_eq!(
            selected.kind,
            winbrew_models::domains::install::InstallerType::Portable
        );

        Ok(())
    }

    #[test]
    fn select_installer_allows_generic_installers_when_platform_is_missing() -> Result<()> {
        let installers = vec![
            sample_installer(
                Architecture::Any,
                winbrew_models::domains::install::InstallerType::Portable,
                None,
            ),
            sample_installer(
                Architecture::X64,
                winbrew_models::domains::install::InstallerType::Msix,
                Some("[\"Windows.Server\"]"),
            ),
        ];

        let selected = select_installer(&installers, normal_host(), Architecture::Arm64)
            .expect("installer should exist");

        assert_eq!(selected.arch, Architecture::Any);
        assert_eq!(
            selected.kind,
            winbrew_models::domains::install::InstallerType::Portable
        );

        Ok(())
    }

    #[test]
    fn select_installer_returns_no_compatible_installer_when_platforms_do_not_match() {
        let installers = vec![sample_installer(
            Architecture::X64,
            winbrew_models::domains::install::InstallerType::Exe,
            Some("[\"Windows.Server\"]"),
        )];

        let err = select_installer(&installers, normal_host(), Architecture::X64)
            .expect_err("installer should not match");

        assert!(matches!(
            err,
            InstallerSelectionError::NoCompatibleInstaller { .. }
        ));
    }

    #[test]
    fn select_installer_returns_no_installers_when_list_is_empty() {
        let err = select_installer(&[], normal_host(), Architecture::X64)
            .expect_err("selection should fail");

        assert!(matches!(err, InstallerSelectionError::NoInstallers));
    }
}
