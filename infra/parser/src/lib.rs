mod error;
mod metadata;
mod parser;
mod pipeline;
mod raw;
mod sqlite;
mod winget;

pub use error::ParserError;
pub use pipeline::{RunConfig, run};
