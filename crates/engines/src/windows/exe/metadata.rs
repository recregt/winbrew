use anyhow::Result;
use std::path::Path;
use tracing::warn;

use crate::windows_dep::fs::paths_refer_to_same_location;
use crate::windows_dep::installed::{UninstallEntry, uninstall_entries_matching};

pub(super) enum NativeExeInstallMetadata {
    QuietOnly(String),
    QuietAndStandard {
        quiet_uninstall_command: String,
        uninstall_command: String,
    },
    StandardOnly(String),
}

pub(super) fn capture_native_exe_metadata(
    package_name: &str,
    install_dir: &Path,
) -> Option<NativeExeInstallMetadata> {
    capture_native_exe_metadata_with(package_name, install_dir, uninstall_entries_matching)
}

pub(super) fn capture_native_exe_metadata_with(
    package_name: &str,
    install_dir: &Path,
    collect_entries: impl FnOnce(&str) -> Result<Vec<UninstallEntry>>,
) -> Option<NativeExeInstallMetadata> {
    let package_name = package_name.trim();
    let mut best_match: Option<(u8, NativeExeInstallMetadata)> = None;
    let mut saw_ambiguous_match = false;

    let Ok(entries) = collect_entries(package_name) else {
        return None;
    };

    for entry in entries {
        if !entry.display_name.trim().eq_ignore_ascii_case(package_name) {
            continue;
        }

        let install_location_exact = match entry.install_location.as_deref() {
            Some(value) if !value.trim().is_empty() => {
                if !paths_refer_to_same_location(Path::new(value), install_dir) {
                    continue;
                }

                true
            }
            _ => false,
        };

        let candidate = match (
            entry.quiet_uninstall_string.as_deref(),
            entry.uninstall_string.as_deref(),
        ) {
            (Some(quiet_uninstall_command), Some(uninstall_command)) => Some((
                native_exe_metadata_priority(install_location_exact, 3),
                NativeExeInstallMetadata::QuietAndStandard {
                    quiet_uninstall_command: quiet_uninstall_command.to_string(),
                    uninstall_command: uninstall_command.to_string(),
                },
            )),
            (Some(quiet_uninstall_command), None) => Some((
                native_exe_metadata_priority(install_location_exact, 2),
                NativeExeInstallMetadata::QuietOnly(quiet_uninstall_command.to_string()),
            )),
            (None, Some(uninstall_command)) => Some((
                native_exe_metadata_priority(install_location_exact, 1),
                NativeExeInstallMetadata::StandardOnly(uninstall_command.to_string()),
            )),
            (None, None) => None,
        };

        let Some((priority, metadata)) = candidate else {
            continue;
        };

        match best_match.as_mut() {
            Some((best_priority, best_metadata)) => {
                if priority > *best_priority {
                    *best_priority = priority;
                    *best_metadata = metadata;
                } else if priority == *best_priority {
                    saw_ambiguous_match = true;
                }
            }
            None => {
                best_match = Some((priority, metadata));
            }
        }
    }

    if saw_ambiguous_match {
        // A tie at the same priority level means install_location couldn't
        // disambiguate (no exact match on either side) and nothing else
        // distinguishes the candidates. Guessing risks capturing an
        // uninstall command from a different, unrelated application that
        // happens to share this display name -- refuse instead, the same
        // way a missing registry entry already results in no metadata and a
        // safe fallback to directory-based removal.
        warn!(
            package = package_name,
            install_dir = %install_dir.display(),
            "multiple native executable uninstall registry entries matched ambiguously; refusing to guess"
        );
        return None;
    }

    best_match.map(|(_, metadata)| metadata)
}

fn native_exe_metadata_priority(install_location_exact: bool, metadata_priority: u8) -> u8 {
    if install_location_exact {
        10 + metadata_priority
    } else {
        metadata_priority
    }
}
