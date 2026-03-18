#[cfg(windows)]
use mimalloc::MiMalloc;

use anyhow::Result;
use clap::Parser;
use winbrew::{Cli, run};

#[cfg(windows)]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> Result<()> {
    let cli = Cli::parse();
    run(cli.command)
}
