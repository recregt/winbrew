//! # Doctor Operations Module
//!
//! Health checks and package integrity verification for the `doctor` command.
//!
//! This module is split into two layers:
//!
//! - [`report`]: assembles a full [`HealthReport`](crate::models::HealthReport)
//! - [`scan`]: validates installed packages and orphaned install directories
//!
//! ## Features
//!
//! - **Parallel scanning**: package checks run on Rayon workers for throughput
//! - **Progress reporting**: optional indicatif progress bars for CLI feedback
//! - **Security policies**: configurable symlink handling and canonical-path control
//! - **Path validation**: rejects empty paths and null-byte input early
//! - **Memory efficiency**: pre-allocated collections for orphan discovery
//!
//! ## Quick Start
//!
//! ```rust
//! use winbrew_app::operations::doctor::{
//!     PackageScanner, ScanConfig, SymlinkPolicy, scan_packages,
//! };
//!
//! # let packages = Vec::new();
//! // Fast path: default package scan.
//! let fast_results = scan_packages(&packages);
//!
//! // Security-aware path: configure symlink handling explicitly.
//! let scanner = PackageScanner::new(
//!     ScanConfig::builder()
//!         .with_symlink_policy(SymlinkPolicy::Deny)
//!         .follow_canonical_paths(true)
//!         .build(),
//! );
//!
//! let secure_results = scanner.scan(&packages);
//! # let _ = (fast_results, secure_results);
//! ```
//!
//! ## Security Notes
//!
//! - Empty install paths are rejected before any filesystem access.
//! - Paths containing null bytes are rejected before any filesystem access.
//! - The configurable scanner can canonicalize paths before metadata checks.
//! - Symlinks can be allowed, warned on, denied, or followed.
//!
//! ## Public API
//!
//! - [`health_report`]
//! - [`scan_packages`]
//! - [`scan_packages_with_progress`]
//! - [`PackageScanner`]
//! - [`ScanConfig`]
//! - [`SymlinkPolicy`]

pub mod report;
pub mod scan;

pub use report::{Reporter, health_report};
pub use scan::{
    PackageScanner, ScanConfig, SymlinkPolicy, installed_packages, scan_orphaned_install_dirs,
    scan_packages, scan_packages_with_progress,
};
