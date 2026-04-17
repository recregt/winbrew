use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("Package catalog not found. Run `winbrew update` to download it.")]
pub struct CatalogNotFoundError;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error(
    "Package catalog schema version mismatch. Expected {expected}, found {actual}. Rebuild the catalog bundle or reparse the source inputs with `winbrew update`."
)]
pub struct CatalogSchemaVersionMismatchError {
    pub expected: u32,
    pub actual: i64,
}
