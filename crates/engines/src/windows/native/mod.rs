//! Windows-native installers live here.

//! This namespace now hosts the MSI engine path. It is separated from archive
//! extractors because MSI installation and removal are driven by the Windows
//! Installer service rather than by file extraction.

pub mod msi;
