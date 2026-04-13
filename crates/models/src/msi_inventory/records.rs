//! Normalized MSI inventory records persisted for repair and doctor flows.

use serde::{Deserialize, Serialize};

use crate::install::InstallScope;
use crate::shared::HashAlgorithm;

/// The MSI inventory receipt stored for a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsiInventoryReceipt {
    /// Package name.
    pub package_name: String,
    /// Product code reported by the MSI.
    pub product_code: String,
    /// Optional upgrade code reported by the MSI.
    pub upgrade_code: Option<String>,
    /// Install scope recorded for the package.
    pub scope: InstallScope,
}

/// A normalized MSI file entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsiFileRecord {
    /// Package name.
    pub package_name: String,
    /// Original file path.
    pub path: String,
    /// Lowercased normalized path used for lookups.
    pub normalized_path: String,
    /// Optional hash algorithm used for the file.
    pub hash_algorithm: Option<HashAlgorithm>,
    /// Hex hash string associated with the file.
    pub hash_hex: Option<String>,
    /// Whether the file originated from a config-related MSI entry.
    pub is_config_file: bool,
}

/// A normalized MSI registry entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsiRegistryRecord {
    /// Package name.
    pub package_name: String,
    /// Registry hive name.
    pub hive: String,
    /// Raw registry key path.
    pub key_path: String,
    /// Lowercased normalized key path used for lookups.
    pub normalized_key_path: String,
    /// Registry value name.
    pub value_name: String,
    /// Registry value data, when present.
    pub value_data: Option<String>,
    /// Previous value captured for repair comparison, when present.
    pub previous_value: Option<String>,
}

/// A normalized MSI shortcut entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsiShortcutRecord {
    /// Package name.
    pub package_name: String,
    /// Shortcut path.
    pub path: String,
    /// Lowercased normalized shortcut path.
    pub normalized_path: String,
    /// Shortcut target path, when present.
    pub target_path: Option<String>,
    /// Lowercased normalized target path, when present.
    pub normalized_target_path: Option<String>,
}

/// A normalized MSI component entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsiComponentRecord {
    /// Package name.
    pub package_name: String,
    /// MSI component identifier.
    pub component_id: String,
    /// Optional component path.
    pub path: Option<String>,
    /// Lowercased normalized component path, when present.
    pub normalized_path: Option<String>,
}

/// The complete MSI inventory snapshot for a package.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsiInventorySnapshot {
    /// Receipt metadata for the package.
    pub receipt: MsiInventoryReceipt,
    /// Normalized file rows.
    pub files: Vec<MsiFileRecord>,
    /// Normalized registry rows.
    pub registry_entries: Vec<MsiRegistryRecord>,
    /// Normalized shortcut rows.
    pub shortcuts: Vec<MsiShortcutRecord>,
    /// Normalized component rows.
    pub components: Vec<MsiComponentRecord>,
}
