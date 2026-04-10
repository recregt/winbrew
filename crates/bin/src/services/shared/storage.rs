use anyhow::Result;

use crate::core::paths::ResolvedPaths;
use crate::database;
use crate::models::{
    CatalogInstaller, CatalogPackage, ConfigSection, ConfigValueSource, Package, PackageStatus,
};

pub use crate::database::{CatalogNotFoundError, PackageNotFoundError};

pub fn init(paths: &ResolvedPaths) -> Result<()> {
    database::init(paths)
}

pub fn get_conn() -> Result<crate::database::DbConnection> {
    database::get_conn()
}

pub fn get_catalog_conn() -> Result<crate::database::DbConnection> {
    database::get_catalog_conn()
}

pub fn list_packages(conn: &crate::database::DbConnection) -> Result<Vec<Package>> {
    database::list_packages(conn)
}

pub fn list_installing_packages(conn: &crate::database::DbConnection) -> Result<Vec<Package>> {
    database::list_installing_packages(conn)
}

pub fn get_package(conn: &crate::database::DbConnection, name: &str) -> Result<Option<Package>> {
    database::get_package(conn, name)
}

pub fn delete_package(conn: &crate::database::DbConnection, name: &str) -> Result<()> {
    database::delete_package(conn, name).map(|_| ())
}

pub fn insert_package(conn: &crate::database::DbConnection, package: &Package) -> Result<()> {
    database::insert_package(conn, package)
}

pub fn update_status(
    conn: &crate::database::DbConnection,
    name: &str,
    status: PackageStatus,
) -> Result<()> {
    database::update_status(conn, name, status)
}

pub fn update_status_and_msix_package_full_name(
    conn: &crate::database::DbConnection,
    name: &str,
    status: PackageStatus,
    msix_package_full_name: Option<&str>,
) -> Result<()> {
    database::update_status_and_msix_package_full_name(conn, name, status, msix_package_full_name)
}

pub fn get_installers(
    conn: &crate::database::DbConnection,
    package_id: &str,
) -> Result<Vec<CatalogInstaller>> {
    database::get_installers(conn, package_id)
}

pub fn get_package_by_id(
    conn: &crate::database::DbConnection,
    package_id: &str,
) -> Result<Option<CatalogPackage>> {
    database::get_package_by_id(conn, package_id)
}

pub fn search(conn: &crate::database::DbConnection, query: &str) -> Result<Vec<CatalogPackage>> {
    database::search(conn, query)
}

pub fn get_effective_value(key: &str) -> Result<(String, ConfigValueSource)> {
    database::get_effective_value(key)
}

pub fn config_set(key: &str, value: &str) -> Result<()> {
    database::config_set(key, value)
}

pub fn config_unset(key: &str) -> Result<()> {
    database::config_unset(key)
}

pub fn config_sections() -> Result<Vec<ConfigSection>> {
    database::config_sections()
}

pub fn load_current_config() -> Result<crate::database::Config> {
    database::Config::load_current()
}
