#[cfg(not(windows))]
compile_error!("winbrew only builds on Windows");

#[cfg(windows)]
use mimalloc::MiMalloc;

use anyhow::Result;

#[cfg(windows)]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

fn main() -> Result<()> {
    winbrew::run_app()
}
