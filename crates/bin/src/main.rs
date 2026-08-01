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
    let verbose = cli.verbose;
    let command = cli.command;

    if let Err(err) = winbrew_cli::run_app(command, verbose) {
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

/// winbrew manages Windows package installs and has no meaningful behavior
/// on other platforms. This stub exists only so the workspace (and every
/// other, genuinely cross-platform crate in it) can be built and tested on
/// non-Windows hosts; it intentionally does not pretend to succeed.
#[cfg(not(windows))]
fn main() -> std::process::ExitCode {
    eprintln!("winbrew is a Windows package manager and does not run on this platform.");
    std::process::ExitCode::FAILURE
}
