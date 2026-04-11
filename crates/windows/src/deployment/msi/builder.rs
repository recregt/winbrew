//! Converts resolved MSI rows into WinBrew inventory records.
//!
//! This layer assumes the database module has already materialized raw MSI
//! tables and the directory module has already collapsed the `Directory`
//! graph into absolute paths. The interesting detail here is that file paths
//! are computed once and reused, because `File`, `Shortcut`, and `Component`
//! records all need to agree on the same derived locations.
//!
//! Registry handling is intentionally narrow. Only `Root = -1` consults the
//! install scope; the other root values are passed through as their concrete
//! registry hives.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use winbrew_models::{
    InstallScope, MsiComponentRecord, MsiFileRecord, MsiRegistryRecord, MsiShortcutRecord,
};

use super::{
    ComponentRow, FileRow, RegistryRow, ShortcutRow,
    path::{normalize_path, normalize_registry_key_path, resolve_reference_path, select_msi_name},
};

pub(super) fn build_file_paths(
    file_rows: &[FileRow],
    component_rows: &HashMap<String, ComponentRow>,
    directory_paths: &HashMap<String, PathBuf>,
    install_root: &Path,
) -> HashMap<String, PathBuf> {
    // Build a cache of derived file paths keyed by the MSI `File` table key.
    //
    // The returned map is used as the canonical source for file records and
    // as a shared lookup for shortcut and component resolution.
    let mut file_paths = HashMap::new();

    for file_row in file_rows {
        file_paths.insert(
            file_row.file_key.clone(),
            resolve_file_row_path(file_row, component_rows, directory_paths, install_root),
        );
    }

    file_paths
}

pub(super) fn build_file_records(
    package_name: &str,
    file_rows: &[FileRow],
    file_paths: &HashMap<String, PathBuf>,
    component_rows: &HashMap<String, ComponentRow>,
    directory_paths: &HashMap<String, PathBuf>,
    install_root: &Path,
) -> Vec<MsiFileRecord> {
    // Convert MSI `File` rows into storage records.
    //
    // If the precomputed file-path cache is missing a key, the code falls
    // back to row-local resolution instead of dropping the record entirely.
    file_rows
        .iter()
        .map(|file_row| {
            let path = file_paths
                .get(&file_row.file_key)
                .cloned()
                .unwrap_or_else(|| {
                    resolve_file_row_path(file_row, component_rows, directory_paths, install_root)
                });

            MsiFileRecord {
                package_name: package_name.to_string(),
                path: path.to_string_lossy().into_owned(),
                normalized_path: normalize_path(&path),
                hash_algorithm: None,
                hash_hex: None,
                is_config_file: false,
            }
        })
        .collect()
}

fn resolve_file_row_path(
    file_row: &FileRow,
    component_rows: &HashMap<String, ComponentRow>,
    directory_paths: &HashMap<String, PathBuf>,
    install_root: &Path,
) -> PathBuf {
    let base_dir = component_rows
        .get(&file_row.component_id)
        .and_then(|component| directory_paths.get(&component.directory_id))
        .cloned()
        .unwrap_or_else(|| install_root.to_path_buf());

    let file_name =
        select_msi_name(&file_row.file_name).unwrap_or_else(|| file_row.file_name.clone());
    base_dir.join(file_name)
}

pub(super) fn build_registry_records(
    package_name: &str,
    scope: InstallScope,
    rows: &[RegistryRow],
) -> Vec<MsiRegistryRecord> {
    // Convert MSI `Registry` rows into normalized storage records.
    //
    // `Root = -1` is the only case that consults `InstallScope`; the other
    // roots map directly to concrete hives.
    rows.iter()
        .map(|row| MsiRegistryRecord {
            package_name: package_name.to_string(),
            hive: registry_root_name(row.root, scope).to_string(),
            key_path: row.key_path.clone(),
            normalized_key_path: normalize_registry_key_path(&row.key_path),
            value_name: row.name.clone().unwrap_or_default(),
            value_data: row.value.clone(),
            previous_value: None,
        })
        .collect()
}

fn registry_root_name(root: i32, scope: InstallScope) -> &'static str {
    match root {
        0 => "HKCR",
        1 => "HKCU",
        2 => "HKLM",
        3 => "HKU",
        -1 => match scope {
            InstallScope::Installed => "HKLM",
            InstallScope::Provisioned => "HKCU",
        },
        _ => "UNKNOWN",
    }
}

pub(super) fn build_shortcut_records(
    package_name: &str,
    rows: &[ShortcutRow],
    directory_paths: &HashMap<String, PathBuf>,
    file_paths: &HashMap<String, PathBuf>,
    install_root: &Path,
) -> Vec<MsiShortcutRecord> {
    // Convert MSI `Shortcut` rows into storage records.
    //
    // Shortcut targets are resolved conservatively: when the target is not a
    // recognizable MSI reference, the target path remains `None` rather than
    // guessing a filesystem location.
    rows.iter()
        .map(|row| {
            let directory_path = directory_paths
                .get(&row.directory_id)
                .cloned()
                .unwrap_or_else(|| install_root.to_path_buf());
            let path =
                directory_path.join(select_msi_name(&row.name).unwrap_or_else(|| row.name.clone()));
            let target_path = resolve_reference_path(&row.target, directory_paths, file_paths);

            MsiShortcutRecord {
                package_name: package_name.to_string(),
                path: path.to_string_lossy().into_owned(),
                normalized_path: normalize_path(&path),
                target_path: target_path
                    .as_ref()
                    .map(|value| value.to_string_lossy().into_owned()),
                normalized_target_path: target_path
                    .as_ref()
                    .map(|value| normalize_path(value.as_path())),
            }
        })
        .collect()
}

pub(super) fn build_component_records(
    package_name: &str,
    component_rows: &HashMap<String, ComponentRow>,
    directory_paths: &HashMap<String, PathBuf>,
    file_paths: &HashMap<String, PathBuf>,
) -> Vec<MsiComponentRecord> {
    // Convert MSI `Component` rows into storage records.
    //
    // A component key path may resolve through either file references or
    // directory references, depending on how the MSI package author encoded
    // the value.
    component_rows
        .iter()
        .map(|(component_id, component)| {
            let path = component
                .key_path
                .as_deref()
                .and_then(|value| resolve_reference_path(value, directory_paths, file_paths));

            MsiComponentRecord {
                package_name: package_name.to_string(),
                component_id: component_id.clone(),
                path: path
                    .as_ref()
                    .map(|value| value.to_string_lossy().into_owned()),
                normalized_path: path.as_ref().map(|value| normalize_path(value.as_path())),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::registry_root_name;
    use winbrew_models::InstallScope;

    #[test]
    fn registry_root_name_uses_scope_for_negative_one() {
        assert_eq!(registry_root_name(-1, InstallScope::Installed), "HKLM");
        assert_eq!(registry_root_name(-1, InstallScope::Provisioned), "HKCU");
    }
}
