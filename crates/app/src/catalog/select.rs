//! Host-aware installer selection policy for catalog packages.
//!
//! Catalog packages can publish multiple installers for different architectures,
//! platform families, and install scopes. This module centralizes the ranking
//! rules so callers do not need to replicate host-profile fallback logic in
//! command code.
//!
//! The selection order is intentionally simple and predictable:
//!
//! - ignore installers whose platform metadata does not match the host family
//! - prefer installers whose scope matches the current elevation state
//! - prefer an installer that exactly matches the host architecture
//! - fall back to `Architecture::Any` when no exact match exists
//! - fall back to the first scope-compatible installer if nothing else matches

use crate::models::domains::catalog::CatalogInstaller;
use crate::models::domains::install::Architecture;
use crate::windows::HostProfile;
use thiserror::Error;
use tracing::debug;

/// Selection inputs derived from the current runtime host state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SelectionContext {
    /// Host family and native architecture snapshot.
    pub host_profile: HostProfile,
    /// `true` when the current process is running elevated.
    pub is_elevated: bool,
}

impl SelectionContext {
    /// Build a new selection context from the current host state.
    pub(crate) fn new(host_profile: HostProfile, is_elevated: bool) -> Self {
        Self {
            host_profile,
            is_elevated,
        }
    }
}

/// Raised when catalog installers cannot be matched to the current host.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub(crate) enum InstallerSelectionError {
    /// The catalog package has no installers at all.
    #[error("catalog package has no installers")]
    NoInstallers,
    /// The package has installers, but none match this host's platform family.
    #[error("no installer matches this host ({host})")]
    PlatformMismatch { host: HostProfile },
    /// The package has installers, but none match this host's install scope.
    #[error("no installer matches this host's install scope ({host})")]
    ScopeMismatch { host: HostProfile },
}

/// Select the best installer for the current host.
///
/// The helper returns a structured error when the catalog package has no
/// installers or when none of the installers are compatible with this host.
pub(crate) fn select_installer(
    installers: &[CatalogInstaller],
    selection_context: SelectionContext,
) -> Result<CatalogInstaller, InstallerSelectionError> {
    if installers.is_empty() {
        return Err(InstallerSelectionError::NoInstallers);
    }

    let compatible_installers: Vec<&CatalogInstaller> = installers
        .iter()
        .filter(|installer| {
            platform_matches_host(
                installer.platform.as_deref(),
                selection_context.host_profile,
            )
        })
        .collect();

    if compatible_installers.is_empty() {
        return Err(InstallerSelectionError::PlatformMismatch {
            host: selection_context.host_profile,
        });
    }

    let scope_compatible_installers =
        scope_compatible_installers(&compatible_installers, selection_context)?;

    debug!(
        installer_count = installers.len(),
        compatible_count = compatible_installers.len(),
        scope_compatible_count = scope_compatible_installers.len(),
        host = %selection_context.host_profile,
        elevated = selection_context.is_elevated,
        "selecting best installer"
    );

    let selected = scope_compatible_installers
        .iter()
        .find(|installer| installer.arch == selection_context.host_profile.architecture)
        .copied()
        .or_else(|| {
            scope_compatible_installers
                .iter()
                .find(|installer| installer.arch == Architecture::Any)
                .copied()
        })
        .or_else(|| scope_compatible_installers.first().copied())
        .expect("compatible installers should not be empty");

    Ok(selected.clone())
}

fn scope_compatible_installers<'a>(
    installers: &'a [&'a CatalogInstaller],
    selection_context: SelectionContext,
) -> Result<Vec<&'a CatalogInstaller>, InstallerSelectionError> {
    let scope_compatible_installers: Vec<&CatalogInstaller> = installers
        .iter()
        .copied()
        .filter(|installer| {
            scope_matches_host(installer.scope.as_deref(), selection_context.is_elevated)
        })
        .collect();

    if scope_compatible_installers.is_empty() {
        return Err(InstallerSelectionError::ScopeMismatch {
            host: selection_context.host_profile,
        });
    }

    if selection_context.is_elevated {
        let machine_installers: Vec<&CatalogInstaller> = scope_compatible_installers
            .iter()
            .copied()
            .filter(|installer| {
                installer_scope_kind(installer.scope.as_deref()) == InstallerScopeKind::Machine
            })
            .collect();

        if machine_installers.is_empty() {
            Ok(scope_compatible_installers)
        } else {
            Ok(machine_installers)
        }
    } else {
        Ok(scope_compatible_installers)
    }
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
        .iter()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .any(|value| {
            accepted_platforms
                .iter()
                .any(|accepted| value.eq_ignore_ascii_case(accepted))
        })
}

fn scope_matches_host(scope: Option<&str>, is_elevated: bool) -> bool {
    !matches!(installer_scope_kind(scope), InstallerScopeKind::Machine) || is_elevated
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstallerScopeKind {
    User,
    Machine,
    Generic,
}

fn installer_scope_kind(scope: Option<&str>) -> InstallerScopeKind {
    let Some(scope) = scope.map(str::trim).filter(|value| !value.is_empty()) else {
        return InstallerScopeKind::Generic;
    };

    if scope.eq_ignore_ascii_case("machine") {
        InstallerScopeKind::Machine
    } else if scope.eq_ignore_ascii_case("user") {
        InstallerScopeKind::User
    } else {
        InstallerScopeKind::Generic
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;

    fn sample_installer(
        arch: Architecture,
        kind: winbrew_models::domains::install::InstallerType,
        platform: Option<&str>,
        scope: Option<&str>,
    ) -> CatalogInstaller {
        let mut installer =
            CatalogInstaller::test_builder("Contoso.App".into(), "https://example.test/app.exe")
                .with_arch(arch)
                .with_kind(kind);

        installer.platform = platform.map(str::to_string);
        installer.scope = scope.map(str::to_string);
        installer
    }

    fn selection_context(
        is_server: bool,
        architecture: Architecture,
        is_elevated: bool,
    ) -> SelectionContext {
        SelectionContext::new(
            HostProfile {
                is_server,
                architecture,
            },
            is_elevated,
        )
    }

    fn normal_host(architecture: Architecture, is_elevated: bool) -> SelectionContext {
        selection_context(false, architecture, is_elevated)
    }

    fn server_host(architecture: Architecture, is_elevated: bool) -> SelectionContext {
        selection_context(true, architecture, is_elevated)
    }

    #[test]
    fn select_installer_prefers_matching_arch_for_normal_hosts() -> Result<()> {
        let installers = vec![
            sample_installer(
                Architecture::Any,
                winbrew_models::domains::install::InstallerType::Portable,
                Some("[\"Windows.Desktop\"]"),
                None,
            ),
            sample_installer(
                Architecture::X64,
                winbrew_models::domains::install::InstallerType::Msix,
                Some("[\"Windows.Desktop\"]"),
                None,
            ),
            sample_installer(
                Architecture::X86,
                winbrew_models::domains::install::InstallerType::Zip,
                Some("[\"Windows.Desktop\"]"),
                None,
            ),
        ];

        let selected = select_installer(&installers, normal_host(Architecture::X64, false))
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
                None,
            ),
            sample_installer(
                Architecture::X64,
                winbrew_models::domains::install::InstallerType::Portable,
                Some("[\"Windows.Server\"]"),
                None,
            ),
            sample_installer(
                Architecture::Any,
                winbrew_models::domains::install::InstallerType::Zip,
                Some("[\"Windows.Server\"]"),
                None,
            ),
        ];

        let selected = select_installer(&installers, server_host(Architecture::X64, false))
            .expect("installer should exist");

        assert_eq!(selected.arch, Architecture::X64);
        assert_eq!(
            selected.kind,
            winbrew_models::domains::install::InstallerType::Portable
        );

        Ok(())
    }

    #[test]
    fn select_installer_accepts_windows_universal_on_normal_hosts() -> Result<()> {
        let installers = vec![sample_installer(
            Architecture::X64,
            winbrew_models::domains::install::InstallerType::Msix,
            Some("[\"WINDOWS.UNIVERSAL\"]"),
            None,
        )];

        let selected = select_installer(&installers, normal_host(Architecture::X64, false))
            .expect("installer should exist");

        assert_eq!(
            selected.kind,
            winbrew_models::domains::install::InstallerType::Msix
        );
        assert_eq!(selected.arch, Architecture::X64);

        Ok(())
    }

    #[test]
    fn select_installer_prefers_machine_scope_when_elevated() -> Result<()> {
        let installers = vec![
            sample_installer(
                Architecture::X64,
                winbrew_models::domains::install::InstallerType::Portable,
                Some("[\"Windows.Universal\"]"),
                Some("user"),
            ),
            sample_installer(
                Architecture::X86,
                winbrew_models::domains::install::InstallerType::Msix,
                Some("[\"Windows.Universal\"]"),
                Some("machine"),
            ),
        ];

        let selected = select_installer(&installers, normal_host(Architecture::X64, true))
            .expect("installer should exist");

        assert_eq!(selected.scope.as_deref(), Some("machine"));
        assert_eq!(selected.arch, Architecture::X86);

        Ok(())
    }

    #[test]
    fn select_installer_allows_generic_installers_when_platform_is_missing() -> Result<()> {
        let installers = vec![
            sample_installer(
                Architecture::Any,
                winbrew_models::domains::install::InstallerType::Portable,
                None,
                None,
            ),
            sample_installer(
                Architecture::X64,
                winbrew_models::domains::install::InstallerType::Msix,
                Some("[\"Windows.Server\"]"),
                None,
            ),
        ];

        let selected = select_installer(&installers, normal_host(Architecture::Arm64, false))
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
            None,
        )];

        let err = select_installer(&installers, normal_host(Architecture::X64, false))
            .expect_err("installer should not match");

        assert_eq!(
            err,
            InstallerSelectionError::PlatformMismatch {
                host: HostProfile {
                    is_server: false,
                    architecture: Architecture::X64,
                },
            }
        );
    }

    #[test]
    fn select_installer_returns_scope_error_for_machine_only_installers_when_not_elevated() {
        let installers = vec![sample_installer(
            Architecture::X64,
            winbrew_models::domains::install::InstallerType::Exe,
            Some("[\"Windows.Universal\"]"),
            Some("machine"),
        )];

        let err = select_installer(&installers, normal_host(Architecture::X64, false))
            .expect_err("installer should not match");

        assert_eq!(
            err,
            InstallerSelectionError::ScopeMismatch {
                host: HostProfile {
                    is_server: false,
                    architecture: Architecture::X64,
                },
            }
        );
    }

    #[test]
    fn select_installer_returns_no_installers_when_list_is_empty() {
        let err = select_installer(&[], normal_host(Architecture::X64, false))
            .expect_err("selection should fail");

        assert!(matches!(err, InstallerSelectionError::NoInstallers));
    }
}
