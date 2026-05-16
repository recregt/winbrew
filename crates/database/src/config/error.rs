use thiserror::Error;

/// Errors raised while validating config values.
///
/// `InvalidValue` in [`ConfigError`] is reserved for raw values that fail to
/// parse or normalize, while `Validation` means the key-specific validator
/// rejected a value that was otherwise structurally valid.
#[derive(Debug, Error)]
pub enum ConfigValidationError {
    /// `core.log_level` is parsed by `tracing_subscriber::EnvFilter`, so the
    /// original parser reason is preserved in the error.
    #[error("invalid core.log_level '{value}': {reason}")]
    InvalidLogLevel { value: String, reason: String },

    /// `core.file_log_level` is parsed by `tracing_subscriber::EnvFilter`, so
    /// the original parser reason is preserved in the error.
    #[error("invalid core.file_log_level '{value}': {reason}")]
    InvalidFileLogLevel { value: String, reason: String },

    #[error("expected a boolean value (true/false, 1/0, yes/no, on/off), got '{value}'")]
    ExpectedBoolean { value: String },
}

/// Errors raised while reading, parsing, or validating config values.
#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("config key cannot be empty")]
    EmptyKey,

    #[error("unknown config key: {key}")]
    UnknownKey { key: String },

    /// The raw value could not be parsed into the target config representation.
    #[error("invalid {key} value: {value}")]
    InvalidValue { key: String, value: String },

    /// The value parsed successfully, but failed a key-specific validator.
    #[error("invalid value for '{key}'")]
    Validation {
        key: String,
        #[source]
        source: ConfigValidationError,
    },
}

pub type ConfigResult<T> = std::result::Result<T, ConfigError>;
