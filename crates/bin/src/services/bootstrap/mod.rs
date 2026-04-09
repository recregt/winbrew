use anyhow::Result;

use crate::cancel;

pub mod cleanup;
pub mod logging;

pub fn init_runtime() -> Result<()> {
    cancel::init_handler()?;
    cleanup::cleanup_stale_installations()?;

    Ok(())
}
