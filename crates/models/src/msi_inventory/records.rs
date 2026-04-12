use serde::{Deserialize, Serialize};

use crate::InstallScope;
use crate::shared::HashAlgorithm;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsiInventoryReceipt {
    pub package_name: String,
    pub product_code: String,
    pub upgrade_code: Option<String>,
    pub scope: InstallScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsiFileRecord {
    pub package_name: String,
    pub path: String,
    pub normalized_path: String,
    pub hash_algorithm: Option<HashAlgorithm>,
    pub hash_hex: Option<String>,
    pub is_config_file: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsiRegistryRecord {
    pub package_name: String,
    pub hive: String,
    pub key_path: String,
    pub normalized_key_path: String,
    pub value_name: String,
    pub value_data: Option<String>,
    pub previous_value: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsiShortcutRecord {
    pub package_name: String,
    pub path: String,
    pub normalized_path: String,
    pub target_path: Option<String>,
    pub normalized_target_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsiComponentRecord {
    pub package_name: String,
    pub component_id: String,
    pub path: Option<String>,
    pub normalized_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MsiInventorySnapshot {
    pub receipt: MsiInventoryReceipt,
    pub files: Vec<MsiFileRecord>,
    pub registry_entries: Vec<MsiRegistryRecord>,
    pub shortcuts: Vec<MsiShortcutRecord>,
    pub components: Vec<MsiComponentRecord>,
}
