use anyhow::{Context, Result, bail};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use winbrew_models::{
    InstallScope, MsiComponentRecord, MsiFileRecord, MsiInventoryReceipt, MsiInventorySnapshot,
    MsiRegistryRecord, MsiShortcutRecord,
};

#[cfg(windows)]
use std::os::windows::ffi::OsStrExt;

#[cfg(windows)]
use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
#[cfg(windows)]
use windows::Win32::System::ApplicationInstallationAndServicing::{
    MSIDBOPEN_READONLY, MSIHANDLE, MsiCloseHandle, MsiDatabaseOpenViewW, MsiOpenDatabaseW,
    MsiRecordGetInteger, MsiRecordGetStringW, MsiRecordIsNull, MsiViewExecute, MsiViewFetch,
};
#[cfg(windows)]
use windows::core::{HSTRING, PCWSTR, PWSTR};

/// Scan an MSI database and reconstruct the inventory snapshot WinBrew stores.
///
/// The scanner reads the standard MSI tables, resolves directory and file keys
/// into concrete install paths rooted at `install_root`, and returns the data in
/// the same snapshot shape used by storage.
pub fn scan_inventory(
    package_path: &Path,
    install_root: &Path,
    package_name: &str,
    scope: InstallScope,
) -> Result<MsiInventorySnapshot> {
    #[cfg(not(windows))]
    {
        let _ = (package_path, install_root, package_name, scope);
        bail!("MSI inventory scanning is only supported on Windows")
    }

    #[cfg(windows)]
    {
        let database = MsiDatabase::open(package_path)?;

        let product_code = query_required_string(
            database.handle(),
            "SELECT `Value` FROM `Property` WHERE `Property` = 'ProductCode'",
        )?;
        let upgrade_code = query_optional_string(
            database.handle(),
            "SELECT `Value` FROM `Property` WHERE `Property` = 'UpgradeCode'",
        )?;

        let directory_rows = load_directory_rows(database.handle())?;
        let component_rows = load_component_rows(database.handle())?;
        let file_rows = load_file_rows(database.handle())?;
        let registry_rows = load_registry_rows(database.handle())?;
        let shortcut_rows = load_shortcut_rows(database.handle())?;

        let directory_paths = resolve_directory_paths(&directory_rows, install_root)?;
        let file_paths =
            build_file_paths(&file_rows, &component_rows, &directory_paths, install_root);

        let files = build_file_records(
            package_name,
            &file_rows,
            &component_rows,
            &directory_paths,
            install_root,
        );
        let registry_entries = build_registry_records(package_name, &registry_rows);
        let shortcuts = build_shortcut_records(
            package_name,
            &shortcut_rows,
            &directory_paths,
            &file_paths,
            install_root,
        );
        let components =
            build_component_records(package_name, &component_rows, &directory_paths, &file_paths);

        Ok(MsiInventorySnapshot {
            receipt: MsiInventoryReceipt {
                package_name: package_name.to_string(),
                product_code,
                upgrade_code,
                scope,
            },
            files,
            registry_entries,
            shortcuts,
            components,
        })
    }
}

#[cfg(windows)]
struct MsiDatabase(MsiHandle);

#[cfg(windows)]
impl MsiDatabase {
    fn open(path: &Path) -> Result<Self> {
        let wide_path = wide_path(path);
        let mut handle = MSIHANDLE(0);
        let status = unsafe {
            MsiOpenDatabaseW(PCWSTR(wide_path.as_ptr()), MSIDBOPEN_READONLY, &mut handle)
        };

        ensure_msi_success(status, "open MSI database")?;

        Ok(Self(MsiHandle::new(handle)))
    }

    fn handle(&self) -> MSIHANDLE {
        self.0.raw()
    }
}

#[cfg(windows)]
#[derive(Debug)]
struct MsiHandle(MSIHANDLE);

#[cfg(windows)]
impl MsiHandle {
    fn new(handle: MSIHANDLE) -> Self {
        Self(handle)
    }

    fn raw(&self) -> MSIHANDLE {
        self.0
    }
}

#[cfg(windows)]
impl Drop for MsiHandle {
    fn drop(&mut self) {
        if self.0.0 != 0 {
            unsafe {
                let _ = MsiCloseHandle(self.0);
            }
        }
    }
}

#[cfg(windows)]
#[derive(Debug, Clone)]
struct DirectoryRow {
    parent: Option<String>,
    default_dir: String,
}

#[cfg(windows)]
#[derive(Debug, Clone)]
struct ComponentRow {
    directory_id: String,
    key_path: Option<String>,
}

#[cfg(windows)]
#[derive(Debug, Clone)]
struct FileRow {
    file_key: String,
    component_id: String,
    file_name: String,
}

#[cfg(windows)]
#[derive(Debug, Clone)]
struct RegistryRow {
    root: i32,
    key_path: String,
    name: Option<String>,
    value: Option<String>,
}

#[cfg(windows)]
#[derive(Debug, Clone)]
struct ShortcutRow {
    directory_id: String,
    name: String,
    target: String,
}

#[cfg(windows)]
fn load_directory_rows(database: MSIHANDLE) -> Result<HashMap<String, DirectoryRow>> {
    let rows = collect_rows(
        database,
        "SELECT `Directory`, `Directory_Parent`, `DefaultDir` FROM `Directory`",
        |record| {
            Ok((
                record_string(record, 1)?,
                DirectoryRow {
                    parent: record_optional_string(record, 2)?,
                    default_dir: record_string(record, 3)?,
                },
            ))
        },
    )?;

    Ok(rows.into_iter().collect())
}

#[cfg(windows)]
fn load_component_rows(database: MSIHANDLE) -> Result<HashMap<String, ComponentRow>> {
    let rows = collect_rows(
        database,
        "SELECT `Component`, `Directory_`, `KeyPath` FROM `Component`",
        |record| {
            Ok((
                record_string(record, 1)?,
                ComponentRow {
                    directory_id: record_string(record, 2)?,
                    key_path: record_optional_string(record, 3)?,
                },
            ))
        },
    )?;

    Ok(rows.into_iter().collect())
}

#[cfg(windows)]
fn load_file_rows(database: MSIHANDLE) -> Result<Vec<FileRow>> {
    collect_rows(
        database,
        "SELECT `File`, `Component_`, `FileName` FROM `File`",
        |record| {
            Ok(FileRow {
                file_key: record_string(record, 1)?,
                component_id: record_string(record, 2)?,
                file_name: record_string(record, 3)?,
            })
        },
    )
}

#[cfg(windows)]
fn load_registry_rows(database: MSIHANDLE) -> Result<Vec<RegistryRow>> {
    collect_rows(
        database,
        "SELECT `Root`, `Key`, `Name`, `Value` FROM `Registry`",
        |record| {
            Ok(RegistryRow {
                root: record_integer(record, 1),
                key_path: record_string(record, 2)?,
                name: record_optional_string(record, 3)?,
                value: record_optional_string(record, 4)?,
            })
        },
    )
}

#[cfg(windows)]
fn load_shortcut_rows(database: MSIHANDLE) -> Result<Vec<ShortcutRow>> {
    collect_rows(
        database,
        "SELECT `Directory_`, `Name`, `Target` FROM `Shortcut`",
        |record| {
            Ok(ShortcutRow {
                directory_id: record_string(record, 1)?,
                name: record_string(record, 2)?,
                target: record_string(record, 3)?,
            })
        },
    )
}

#[cfg(windows)]
fn build_file_paths(
    file_rows: &[FileRow],
    component_rows: &HashMap<String, ComponentRow>,
    directory_paths: &HashMap<String, PathBuf>,
    install_root: &Path,
) -> HashMap<String, PathBuf> {
    let mut file_paths = HashMap::new();

    for file_row in file_rows {
        let base_dir = component_rows
            .get(&file_row.component_id)
            .and_then(|component| directory_paths.get(&component.directory_id))
            .cloned()
            .unwrap_or_else(|| install_root.to_path_buf());

        let file_name =
            select_msi_name(&file_row.file_name).unwrap_or_else(|| file_row.file_name.clone());
        file_paths.insert(file_row.file_key.clone(), base_dir.join(file_name));
    }

    file_paths
}

#[cfg(windows)]
fn build_file_records(
    package_name: &str,
    file_rows: &[FileRow],
    component_rows: &HashMap<String, ComponentRow>,
    directory_paths: &HashMap<String, PathBuf>,
    install_root: &Path,
) -> Vec<MsiFileRecord> {
    file_rows
        .iter()
        .map(|file_row| {
            let base_dir = component_rows
                .get(&file_row.component_id)
                .and_then(|component| directory_paths.get(&component.directory_id))
                .cloned()
                .unwrap_or_else(|| install_root.to_path_buf());

            let file_name =
                select_msi_name(&file_row.file_name).unwrap_or_else(|| file_row.file_name.clone());
            let path = base_dir.join(file_name);

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

#[cfg(windows)]
fn build_registry_records(package_name: &str, rows: &[RegistryRow]) -> Vec<MsiRegistryRecord> {
    rows.iter()
        .map(|row| MsiRegistryRecord {
            package_name: package_name.to_string(),
            hive: registry_root_name(row.root).to_string(),
            key_path: row.key_path.clone(),
            normalized_key_path: normalize_registry_key_path(&row.key_path),
            value_name: row.name.clone().unwrap_or_default(),
            value_data: row.value.clone(),
            previous_value: None,
        })
        .collect()
}

#[cfg(windows)]
fn build_shortcut_records(
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

#[cfg(windows)]
fn build_component_records(
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

#[cfg(windows)]
fn resolve_directory_paths(
    rows: &HashMap<String, DirectoryRow>,
    install_root: &Path,
) -> Result<HashMap<String, PathBuf>> {
    let mut resolved = HashMap::new();
    let mut visiting = HashSet::new();

    for directory_id in rows.keys() {
        let _ = resolve_directory_path(
            directory_id,
            rows,
            &mut resolved,
            &mut visiting,
            install_root,
        )?;
    }

    Ok(resolved)
}

#[cfg(windows)]
fn resolve_directory_path(
    directory_id: &str,
    rows: &HashMap<String, DirectoryRow>,
    resolved: &mut HashMap<String, PathBuf>,
    visiting: &mut HashSet<String>,
    install_root: &Path,
) -> Result<PathBuf> {
    if let Some(path) = resolved.get(directory_id) {
        return Ok(path.clone());
    }

    if !visiting.insert(directory_id.to_string()) {
        bail!("cycle detected in MSI Directory table at '{directory_id}'");
    }

    let row = rows
        .get(directory_id)
        .with_context(|| format!("missing MSI Directory row for '{directory_id}'"))?;

    let base = match row.parent.as_deref() {
        Some(parent) if !parent.is_empty() => {
            resolve_directory_path(parent, rows, resolved, visiting, install_root)?
        }
        _ if directory_id.eq_ignore_ascii_case("TARGETDIR")
            || directory_id.eq_ignore_ascii_case("SOURCEDIR") =>
        {
            install_root.to_path_buf()
        }
        _ => install_root.to_path_buf(),
    };

    let path = match select_msi_name(&row.default_dir) {
        Some(segment) => base.join(segment),
        None => base,
    };

    visiting.remove(directory_id);
    resolved.insert(directory_id.to_string(), path.clone());

    Ok(path)
}

#[cfg(windows)]
fn resolve_reference_path(
    reference: &str,
    directory_paths: &HashMap<String, PathBuf>,
    file_paths: &HashMap<String, PathBuf>,
) -> Option<PathBuf> {
    let reference = reference.trim();
    if reference.is_empty() {
        return None;
    }

    if let Some(key) = reference
        .strip_prefix("[#")
        .and_then(|value| value.strip_suffix(']'))
    {
        return file_paths
            .get(key)
            .cloned()
            .or_else(|| directory_paths.get(key).cloned());
    }

    if let Some(rest) = reference.strip_prefix('[')
        && let Some((key, suffix)) = rest.split_once(']')
    {
        let base = file_paths
            .get(key)
            .cloned()
            .or_else(|| directory_paths.get(key).cloned())?;
        let suffix = suffix.trim_start_matches(['\\', '/']);

        return Some(if suffix.is_empty() {
            base
        } else {
            base.join(suffix)
        });
    }

    if let Some(path) = file_paths.get(reference) {
        return Some(path.clone());
    }

    if let Some(path) = directory_paths.get(reference) {
        return Some(path.clone());
    }

    if reference.contains('\\') || reference.contains('/') || reference.contains(':') {
        return Some(PathBuf::from(reference));
    }

    None
}

#[cfg(windows)]
fn select_msi_name(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value == "." {
        return None;
    }

    let selected = match value.split_once('|') {
        Some((short_name, long_name)) => {
            let long_name = long_name.trim();
            let short_name = short_name.trim();

            if !long_name.is_empty() && long_name != "." {
                long_name
            } else if !short_name.is_empty() && short_name != "." {
                short_name
            } else {
                return None;
            }
        }
        None => value,
    };

    if selected.is_empty() || selected == "." {
        None
    } else {
        Some(selected.to_string())
    }
}

#[cfg(windows)]
fn registry_root_name(root: i32) -> &'static str {
    match root {
        0 => "HKCR",
        1 => "HKCU",
        2 => "HKLM",
        3 => "HKU",
        _ => "UNKNOWN",
    }
}

#[cfg(windows)]
fn normalize_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let stripped = raw
        .strip_prefix(r"\\?\UNC\")
        .map(|value| format!(r"\\{}", value))
        .or_else(|| raw.strip_prefix(r"\\?\").map(ToOwned::to_owned))
        .unwrap_or_else(|| raw.to_string());

    stripped.replace('\\', "/").to_ascii_lowercase()
}

#[cfg(windows)]
fn normalize_registry_key_path(path: &str) -> String {
    path.trim().to_ascii_lowercase()
}

#[cfg(windows)]
fn query_required_string(database: MSIHANDLE, query: &str) -> Result<String> {
    query_optional_string(database, query)?
        .with_context(|| format!("missing MSI query result for '{query}'"))
}

#[cfg(windows)]
fn query_optional_string(database: MSIHANDLE, query: &str) -> Result<Option<String>> {
    let rows = collect_rows(database, query, |record| record_string(record, 1))?;

    Ok(rows.into_iter().next())
}

#[cfg(windows)]
fn collect_rows<T, F>(database: MSIHANDLE, query: &str, mut parse_row: F) -> Result<Vec<T>>
where
    F: FnMut(MSIHANDLE) -> Result<T>,
{
    let view = open_view(database, query)?;
    let view = MsiHandle::new(view);
    execute_view(view.raw())?;

    let mut rows = Vec::new();

    loop {
        let mut record = MSIHANDLE(0);
        let status = unsafe { MsiViewFetch(view.raw(), &mut record) };

        if status == ERROR_NO_MORE_ITEMS.0 || record.0 == 0 {
            break;
        }

        ensure_msi_success(status, "fetch MSI record")?;

        let record = MsiHandle::new(record);
        rows.push(parse_row(record.raw())?);
    }

    Ok(rows)
}

#[cfg(windows)]
fn open_view(database: MSIHANDLE, query: &str) -> Result<MSIHANDLE> {
    let query = HSTRING::from(query);
    let mut view = MSIHANDLE(0);
    let status = unsafe { MsiDatabaseOpenViewW(database, &query, &mut view) };

    ensure_msi_success(status, "open MSI view")?;

    Ok(view)
}

#[cfg(windows)]
fn execute_view(view: MSIHANDLE) -> Result<()> {
    let status = unsafe { MsiViewExecute(view, MSIHANDLE(0)) };

    ensure_msi_success(status, "execute MSI view")
}

#[cfg(windows)]
fn record_optional_string(record: MSIHANDLE, field: u32) -> Result<Option<String>> {
    if unsafe { MsiRecordIsNull(record, field).as_bool() } {
        return Ok(None);
    }

    record_string(record, field).map(Some)
}

#[cfg(windows)]
fn record_string(record: MSIHANDLE, field: u32) -> Result<String> {
    let mut probe = [0u16; 1];
    let mut length = 0u32;
    let status = unsafe {
        MsiRecordGetStringW(
            record,
            field,
            Some(PWSTR(probe.as_mut_ptr())),
            Some(&mut length),
        )
    };

    if status == ERROR_SUCCESS.0 {
        return Ok(String::new());
    }

    if status != ERROR_MORE_DATA.0 {
        ensure_msi_success(status, "probe MSI record string")?;
    }

    let mut buffer = vec![0u16; length as usize + 1];
    let mut written = buffer.len() as u32;
    let status = unsafe {
        MsiRecordGetStringW(
            record,
            field,
            Some(PWSTR(buffer.as_mut_ptr())),
            Some(&mut written),
        )
    };
    ensure_msi_success(status, "read MSI record string")?;

    buffer.truncate(written as usize);
    Ok(String::from_utf16_lossy(&buffer))
}

#[cfg(windows)]
fn record_integer(record: MSIHANDLE, field: u32) -> i32 {
    unsafe { MsiRecordGetInteger(record, field) }
}

#[cfg(windows)]
fn ensure_msi_success(status: u32, context: &str) -> Result<()> {
    if status == ERROR_SUCCESS.0 {
        Ok(())
    } else {
        bail!("{context} failed with MSI error code {status}")
    }
}

#[cfg(windows)]
fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}

#[cfg(test)]
mod tests {
    use super::{normalize_path, normalize_registry_key_path, select_msi_name};
    use std::path::Path;

    #[test]
    fn select_msi_name_prefers_long_name() {
        assert_eq!(
            select_msi_name("SHORT|Long Name"),
            Some("Long Name".to_string())
        );
    }

    #[test]
    fn select_msi_name_handles_plain_values() {
        assert_eq!(
            select_msi_name("FolderName"),
            Some("FolderName".to_string())
        );
        assert_eq!(select_msi_name("."), None);
        assert_eq!(select_msi_name(""), None);
    }

    #[test]
    fn normalize_path_lowercases_and_uses_forward_slashes() {
        assert_eq!(
            normalize_path(Path::new(r"C:\Tools\Demo\bin\App.EXE")),
            "c:/tools/demo/bin/app.exe"
        );
    }

    #[test]
    fn normalize_registry_key_path_lowercases() {
        assert_eq!(
            normalize_registry_key_path(r"Software\Demo\Config"),
            "software\\demo\\config"
        );
    }
}
