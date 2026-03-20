#[cfg(windows)]
use mimalloc::MiMalloc;

use anyhow::Result;
use clap::Parser;
use tracing_subscriber::EnvFilter;
use winbrew::{Cli, run};

#[cfg(windows)]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .without_time()
        .init();

    let cli = Cli::parse();
    run(cli.command)
}
