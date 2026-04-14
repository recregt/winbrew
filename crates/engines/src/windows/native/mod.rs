//! Windows-native installers live here.
//!
//! This namespace hosts the MSI and native-executable engine paths. They are
//! separated from archive extractors because Windows Installer-driven flows and
//! native bootstrapper flows are coordinated through process execution rather
//! than file extraction.

pub mod exe;
pub mod msi;
