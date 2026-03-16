mod cli;
mod cleaner;
mod registry;
mod scanner;
mod uninstall;

use anyhow::Result;

fn main() -> Result<()> {
    cli::Cli::run()
}
