use anyhow::Result;

use crate::core::cancel;

use super::stale_cleanup;

pub fn init_runtime() -> Result<()> {
    cancel::init_handler()?;
    stale_cleanup::cleanup_stale_installations()?;

    Ok(())
}
