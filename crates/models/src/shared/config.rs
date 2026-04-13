//! Configuration view models and value provenance.
//!
//! These types are the presentation layer for configuration data. They do not
//! read or write configuration themselves; they describe how a setting should be
//! displayed and where the current value came from.

/// A rendered configuration section with key/value pairs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigSection {
    /// Human-readable section title.
    pub title: String,
    /// Rendered key/value entries for the section.
    pub entries: Vec<(String, String)>,
}

/// A configuration value paired with its source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigValue {
    /// The resolved configuration value.
    pub value: String,
    /// Whether the value came from the environment or the config file.
    pub source: ConfigValueSource,
}

/// The provenance of a resolved configuration value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigValueSource {
    /// Value came from an environment variable override.
    Env,
    /// Value came from the persisted config file.
    File,
}
