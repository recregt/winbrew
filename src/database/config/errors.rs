use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigValidationError {
    #[error("invalid core.log_level value '{value}'; allowed values: {allowed}")]
    InvalidLogLevel { value: String, allowed: String },

    #[error("invalid core.file_log_level '{value}': {reason}")]
    InvalidFileLogLevel { value: String, reason: String },

    #[error("expected a boolean value (true/false), got '{value}'")]
    ExpectedBoolean { value: String },
}

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
        source: ConfigValidationError,
    },
}

pub type ConfigResult<T> = std::result::Result<T, ConfigError>;
