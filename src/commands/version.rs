use anyhow::Result;

use crate::services::version;

pub fn run() -> Result<()> {
    println!("{}", version::version_string());
    Ok(())
}
