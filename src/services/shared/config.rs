use anyhow::Result;

use super::storage;

pub fn load_current() -> Result<crate::database::Config> {
    storage::load_current_config()
}
