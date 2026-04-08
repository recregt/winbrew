use anyhow::Result;

use crate::runtime::cancel;

pub mod cleanup;

pub fn init_runtime() -> Result<()> {
    cancel::init_handler()?;
    cleanup::cleanup_stale_installations()?;

    Ok(())
}
