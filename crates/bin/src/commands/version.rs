use anyhow::Result;

use crate::AppContext;
use crate::services::app::version;

pub fn run(_ctx: &AppContext) -> Result<()> {
    println!("{}", version::version_string());
    Ok(())
}
