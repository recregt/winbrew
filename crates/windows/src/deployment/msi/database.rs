use anyhow::{Context, Result, bail};
use std::collections::HashMap;
use std::os::windows::ffi::OsStrExt;
use std::path::Path;
use windows::Win32::Foundation::{ERROR_MORE_DATA, ERROR_NO_MORE_ITEMS, ERROR_SUCCESS};
use windows::Win32::System::ApplicationInstallationAndServicing::{
    MSIDBOPEN_READONLY, MSIHANDLE, MsiCloseHandle, MsiDatabaseOpenViewW, MsiOpenDatabaseW,
    MsiRecordGetInteger, MsiRecordGetStringW, MsiRecordIsNull, MsiViewExecute, MsiViewFetch,
};
use windows::core::{HSTRING, PCWSTR, PWSTR};

use super::{ComponentRow, DirectoryRow, FileRow, RegistryRow, ShortcutRow};

pub(super) struct MsiDatabase(MsiHandle);

impl MsiDatabase {
    pub(super) fn open(path: &Path) -> Result<Self> {
        let wide_path = wide_path(path);
        let mut handle = MSIHANDLE(0);
        let status = unsafe {
            MsiOpenDatabaseW(PCWSTR(wide_path.as_ptr()), MSIDBOPEN_READONLY, &mut handle)
        };

        ensure_msi_success(status, "open MSI database")?;

        Ok(Self(MsiHandle::new(handle)))
    }

    pub(super) fn handle(&self) -> MSIHANDLE {
        self.0.raw()
    }
}

#[derive(Debug)]
struct MsiHandle(MSIHANDLE);

impl MsiHandle {
    fn new(handle: MSIHANDLE) -> Self {
        Self(handle)
    }

    fn raw(&self) -> MSIHANDLE {
        self.0
    }
}

impl Drop for MsiHandle {
    fn drop(&mut self) {
        if self.0.0 != 0 {
            unsafe {
                let _ = MsiCloseHandle(self.0);
            }
        }
    }
}

pub(super) fn load_directory_rows(database: MSIHANDLE) -> Result<HashMap<String, DirectoryRow>> {
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

pub(super) fn load_component_rows(database: MSIHANDLE) -> Result<HashMap<String, ComponentRow>> {
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

pub(super) fn load_file_rows(database: MSIHANDLE) -> Result<Vec<FileRow>> {
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

pub(super) fn load_registry_rows(database: MSIHANDLE) -> Result<Vec<RegistryRow>> {
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

pub(super) fn load_shortcut_rows(database: MSIHANDLE) -> Result<Vec<ShortcutRow>> {
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

pub(super) fn query_required_string(database: MSIHANDLE, query: &str) -> Result<String> {
    query_optional_string(database, query)?
        .with_context(|| format!("missing MSI query result for '{query}'"))
}

pub(super) fn query_optional_string(database: MSIHANDLE, query: &str) -> Result<Option<String>> {
    let rows = collect_rows(database, query, |record| record_string(record, 1))?;

    Ok(rows.into_iter().next())
}

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

fn open_view(database: MSIHANDLE, query: &str) -> Result<MSIHANDLE> {
    let query = HSTRING::from(query);
    let mut view = MSIHANDLE(0);
    let status = unsafe { MsiDatabaseOpenViewW(database, &query, &mut view) };

    ensure_msi_success(status, "open MSI view")?;

    Ok(view)
}

fn execute_view(view: MSIHANDLE) -> Result<()> {
    let status = unsafe { MsiViewExecute(view, MSIHANDLE(0)) };

    ensure_msi_success(status, "execute MSI view")
}

fn record_optional_string(record: MSIHANDLE, field: u32) -> Result<Option<String>> {
    if unsafe { MsiRecordIsNull(record, field).as_bool() } {
        return Ok(None);
    }

    record_string(record, field).map(Some)
}

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
    String::from_utf16(&buffer).context("MSI record string contained invalid UTF-16")
}

fn record_integer(record: MSIHANDLE, field: u32) -> i32 {
    unsafe { MsiRecordGetInteger(record, field) }
}

fn ensure_msi_success(status: u32, context: &str) -> Result<()> {
    if status == ERROR_SUCCESS.0 {
        Ok(())
    } else {
        bail!("{context} failed with MSI error code {status}")
    }
}

fn wide_path(path: &Path) -> Vec<u16> {
    path.as_os_str().encode_wide().chain(Some(0)).collect()
}
