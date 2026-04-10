#[cfg(windows)]
use clap::Parser;

#[cfg(windows)]
use mimalloc::MiMalloc;

#[cfg(windows)]
use std::error::Error as _;

#[cfg(windows)]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(windows)]
fn main() -> std::process::ExitCode {
    let cli = winbrew_cli::cli::Cli::parse();

    if let Err(err) = winbrew_cli::run_app(cli.command) {
        if let Some(cmd_err) = err.downcast_ref::<winbrew_cli::commands::error::CommandError>() {
            if let winbrew_cli::commands::error::CommandError::Fatal(message) = cmd_err {
                eprintln!("\nFATAL: {message}");
            }

            if cli.verbose > 0 {
                print_command_error_sources(cmd_err);
            }

            return std::process::ExitCode::from(cmd_err);
        }

        eprintln!("\nUNEXPECTED: {err:#}");
        return std::process::ExitCode::from(1);
    }

    std::process::ExitCode::SUCCESS
}

#[cfg(windows)]
fn print_command_error_sources(err: &winbrew_cli::commands::error::CommandError) {
    let Some(mut source) = err.source() else {
        return;
    };

    eprintln!("Caused by:");
    loop {
        eprintln!("  - {source}");

        match source.source() {
            Some(next) => source = next,
            None => break,
        }
    }
}
