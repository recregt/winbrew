//! MSI inventory snapshot records.
//!
//! MSI inventory is the persisted repair substrate used by install, doctor,
//! and storage code. The records here are intentionally normalized and
//! filesystem-oriented so they can round-trip through the database without
//! losing the original MSI semantics.

pub mod records;

pub use records::*;
