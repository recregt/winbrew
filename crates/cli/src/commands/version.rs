use anyhow::Result;

use crate::{AppContext, app::version};

pub fn run(_ctx: &AppContext) -> Result<()> {
    println!("{}", version::version_string());
    Ok(())
}
