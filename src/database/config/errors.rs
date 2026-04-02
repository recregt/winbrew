use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config key cannot be empty")]
    EmptyKey,

    #[error("unknown config key: {key}")]
    UnknownKey { key: String },

    #[error("invalid {key} value: {value}")]
    InvalidValue { key: String, value: String },

    #[error("invalid value for '{key}'")]
    Validation {
        key: String,
        #[source]
        source: anyhow::Error,
    },
}

pub type ConfigResult<T> = std::result::Result<T, ConfigError>;