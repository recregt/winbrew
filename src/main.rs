#[cfg(windows)]
use mimalloc::MiMalloc;

use anyhow::Result;
use clap::Parser;
use winbrew::core::logging;
use winbrew::{Cli, database, run};

#[cfg(windows)]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> Result<()> {
    logging::init()?;

    let cli = Cli::parse();

    database::init()?;

    run(cli.command)
}
