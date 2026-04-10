#[cfg(windows)]
use mimalloc::MiMalloc;

#[cfg(windows)]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(windows)]
fn main() -> anyhow::Result<()> {
    if let Err(err) = winbrew::run_app() {
        if let Some(cmd_err) = err.downcast_ref::<winbrew::commands::command_errors::CommandError>()
        {
            if let winbrew::commands::command_errors::CommandError::Fatal(message) = cmd_err {
                eprintln!("\nFATAL: {message}");
            }

            std::process::exit(cmd_err.exit_code());
        }

        eprintln!("\nUNEXPECTED: {err:#}");
        std::process::exit(1);
    }

    Ok(())
}
