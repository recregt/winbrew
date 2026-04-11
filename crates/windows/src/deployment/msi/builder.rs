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
