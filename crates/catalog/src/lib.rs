#![cfg(windows)]

pub use winbrew_core as core;
pub use winbrew_storage as storage;

pub mod catalog;
pub mod update;

pub use catalog::*;
pub use update::refresh_catalog;
