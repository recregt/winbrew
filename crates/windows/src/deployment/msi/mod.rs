#![cfg(windows)]
#![doc = include_str!("README.md")]

use anyhow::Result;
use std::path::Path;

use crate::models::install::engine::InstallScope;
use crate::models::msi_inventory::records::{MsiInventoryReceipt, MsiInventorySnapshot};

mod builder;
mod database;
mod directory;
mod path;

use self::{
    builder::{
        build_component_records, build_file_paths, build_file_records, build_registry_records,
        build_shortcut_records,
    },
    database::{
        MsiDatabase, load_component_rows, load_directory_rows, load_file_rows, load_registry_rows,
        load_shortcut_rows, query_optional_string, query_required_string,
    },
    directory::resolve_directory_paths,
};

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
    let file_paths = build_file_paths(&file_rows, &component_rows, &directory_paths, install_root);

    let files = build_file_records(
        package_name,
        &file_rows,
        &file_paths,
        &component_rows,
        &directory_paths,
        install_root,
    );
    let registry_entries = build_registry_records(package_name, scope, &registry_rows);
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

#[derive(Debug, Clone)]
struct DirectoryRow {
    parent: Option<String>,
    default_dir: String,
}

#[derive(Debug, Clone)]
struct ComponentRow {
    directory_id: String,
    key_path: Option<String>,
}

#[derive(Debug, Clone)]
struct FileRow {
    file_key: String,
    component_id: String,
    file_name: String,
}

#[derive(Debug, Clone)]
struct RegistryRow {
    root: i32,
    key_path: String,
    name: Option<String>,
    value: Option<String>,
}

#[derive(Debug, Clone)]
struct ShortcutRow {
    directory_id: String,
    name: String,
    target: String,
}
