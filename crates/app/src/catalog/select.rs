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
use crate::windows::HostProfile;
use thiserror::Error;
use tracing::debug;

/// Raised when catalog installers cannot be matched to the current host.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallerSelectionError {
    /// The catalog package has no installers at all.
    #[error("catalog package has no installers")]
    NoInstallers,
    /// The package has installers, but none match this host's platform family.
    #[error("no installer matches this host ({host})")]
    NoCompatibleInstaller { host: HostProfile },
}

/// Select the best installer for the current architecture.
///
/// The helper returns a structured error when the catalog package has no
/// installers or when none of the installers are compatible with this host.
pub(crate) fn select_installer(
    installers: &[CatalogInstaller],
    host_profile: HostProfile,
) -> Result<CatalogInstaller, InstallerSelectionError> {
    if installers.is_empty() {
        return Err(InstallerSelectionError::NoInstallers);
    }

    let compatible_installers: Vec<&CatalogInstaller> = installers
        .iter()
        .filter(|installer| platform_matches_host(installer.platform.as_deref(), host_profile))
        .collect();

    if compatible_installers.is_empty() {
        return Err(InstallerSelectionError::NoCompatibleInstaller { host: host_profile });
    }

    debug!(
        installer_count = installers.len(),
        compatible_count = compatible_installers.len(),
        host = %host_profile,
        "selecting best installer"
    );

    let selected = compatible_installers
        .iter()
        .find(|installer| installer.arch == host_profile.architecture)
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

fn platform_matches_host(platform: Option<&str>, host_profile: HostProfile) -> bool {
    let Some(platform) = platform else {
        return true;
    };

    let Ok(platform_values) = serde_json::from_str::<Vec<String>>(platform) else {
        return false;
    };

    let accepted_platforms = host_profile.platform_tags();

    platform_values
        .into_iter()
        .map(|value| value.trim().to_ascii_lowercase())
        .any(|value| accepted_platforms.iter().any(|accepted| value == *accepted))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::windows::HostProfile;
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

    fn normal_host(architecture: Architecture) -> HostProfile {
        HostProfile {
            is_server: false,
            architecture,
        }
    }

    fn server_host(architecture: Architecture) -> HostProfile {
        HostProfile {
            is_server: true,
            architecture,
        }
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

        let selected = select_installer(&installers, normal_host(Architecture::X64))
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

        let selected = select_installer(&installers, server_host(Architecture::X64))
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

        let selected = select_installer(&installers, normal_host(Architecture::Arm64))
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

        let err = select_installer(&installers, normal_host(Architecture::X64))
            .expect_err("installer should not match");

        assert_eq!(
            err,
            InstallerSelectionError::NoCompatibleInstaller {
                host: normal_host(Architecture::X64),
            }
        );
    }

    #[test]
    fn select_installer_returns_no_installers_when_list_is_empty() {
        let err = select_installer(&[], normal_host(Architecture::X64))
            .expect_err("selection should fail");

        assert!(matches!(err, InstallerSelectionError::NoInstallers));
    }
}
