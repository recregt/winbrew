//! Version command wrapper for the binary version string.

use anyhow::Result;

use crate::{CommandContext, app::version};

pub fn run(_ctx: &CommandContext) -> Result<()> {
    println!("{}", version::version_string());
    Ok(())
}
