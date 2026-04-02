use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
#[error("Package catalog not found. Run `winbrew update` to download it.")]
pub struct CatalogNotFoundError;