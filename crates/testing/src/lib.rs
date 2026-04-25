#![cfg(windows)]

//! Shared test harness for WinBrew workspace crates.
//!
//! This crate keeps test-only helpers, fixtures, and subprocess wrappers out of
//! the production dependency graph. Production crates should only reference it
//! from `[dev-dependencies]`.

pub use winbrew_core as core;
pub use winbrew_database as database;
pub use winbrew_models as models;

pub mod catalog;
pub mod db;
pub mod env;
pub mod font;
pub mod mock;
pub mod output;
pub mod package;
pub mod shared_root;
pub mod zip;

pub use catalog::CatalogInstallerSeed;
pub use catalog::{
    CatalogInstallerBuilderExt, CatalogPackageBuilderExt, append_catalog_db,
    append_catalog_db_with_installer, catalog_installer, catalog_package, catalog_package_id,
    seed_catalog_db, seed_catalog_db_with_installer, seed_catalog_db_with_installers,
    seed_catalog_package, seed_catalog_package_with_installer,
    seed_catalog_package_with_installers,
};
pub use db::{init_database, reset_install_state, reset_installed_packages};
pub use env::{TestEnvVar, env_lock};
pub use font::{system_font_file_name, system_font_path};
pub use mock::MockServer;
pub use mockito::Matcher;
pub use mockito::Mock;
pub use output::{
    assert_output_contains, assert_output_contains_all, assert_success, output_text, repo_root,
    run_winbrew,
};
pub use package::{DEFAULT_INSTALLED_AT, InstalledPackageBuilder};
pub use shared_root::test_root;
pub use zip::{create_dummy_zip_bytes, digest_hex, md5_hex, sha1_hex, sha512_hex};
