use anyhow::Result;

use crate::core::cancel;

use super::install::recovery;

pub fn init_runtime() -> Result<()> {
    cancel::init_handler()?;
    recovery::recover_stale_installations()?;

    Ok(())
}
