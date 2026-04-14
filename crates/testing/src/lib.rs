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
pub mod mock;
pub mod output;
pub mod package;
pub mod shared_root;
pub mod zip;

pub use catalog::{append_catalog_db, catalog_package_id, seed_catalog_db, seed_catalog_package};
pub use db::{init_database, reset_install_state, reset_installed_packages};
pub use env::{TestEnvVar, env_lock};
pub use mock::MockServer;
pub use mockito::Mock;
pub use output::{
    assert_output_contains, assert_output_contains_all, assert_success, output_text, repo_root,
    run_winbrew,
};
pub use package::{DEFAULT_INSTALLED_AT, InstalledPackageBuilder};
pub use shared_root::test_root;
pub use zip::{create_dummy_zip_bytes, digest_hex, md5_hex, sha1_hex, sha512_hex};
