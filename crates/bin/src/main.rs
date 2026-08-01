#[cfg(windows)]
use {
    clap::Parser,
    mimalloc::MiMalloc,
    std::error::Error as _,
    std::process::ExitCode,
    winbrew_cli::{cli::Cli, commands::error::CommandError, run_app},
};

#[cfg(windows)]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(windows)]
fn main() -> ExitCode {
    let cli = Cli::parse();
    let verbose = cli.verbose;

    let Err(err) = run_app(cli.command, verbose) else {
        return ExitCode::SUCCESS;
    };

    let Some(cmd_err) = err.downcast_ref::<CommandError>() else {
        eprintln!("\nUNEXPECTED: {err:#}");
        return ExitCode::from(1);
    };

    if let CommandError::Fatal(message) = cmd_err {
        eprintln!("\nFATAL: {message}");
    }
    if verbose > 0 {
        print_command_error_sources(cmd_err);
    }

    ExitCode::from(cmd_err)
}

#[cfg(windows)]
fn print_command_error_sources(err: &CommandError) {
    let Some(mut source) = err.source() else {
        return;
    };

    eprintln!("Caused by:");
    loop {
        eprintln!("  - {source}");
        let Some(next) = source.source() else {
            break;
        };
        source = next;
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
