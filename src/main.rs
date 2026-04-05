#[cfg(not(windows))]
compile_error!("winbrew only builds on Windows");

#[cfg(windows)]
use mimalloc::MiMalloc;

#[cfg(windows)]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    winbrew::run_app()
}
