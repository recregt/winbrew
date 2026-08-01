use std::process::ExitCode;

/// The real CLI. Living in its own `#[cfg(windows)]` module means every item
/// inside inherits the platform gate for free instead of repeating
/// `#[cfg(windows)]` on each import, the allocator, and every function.
#[cfg(windows)]
mod platform {
    use std::process::ExitCode;

    use clap::Parser;
    use mimalloc::MiMalloc;
    use winbrew_cli::{cli::Cli, commands::error::CommandError, run_app};

    #[global_allocator]
    static GLOBAL: MiMalloc = MiMalloc;

    pub fn main() -> ExitCode {
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

        // `err.chain()` yields `err` itself first, then each `source()` in
        // turn, so skip(1) is everything cmd_err was ultimately caused by.
        // Peek first so a bare "Caused by:" header never prints alone.
        let mut causes = err.chain().skip(1).peekable();
        if verbose > 0 && causes.peek().is_some() {
            eprintln!("Caused by:");
            for cause in causes {
                eprintln!("  - {cause}");
            }
        }

        ExitCode::from(cmd_err)
    }
}

/// winbrew manages Windows package installs and has no meaningful behavior
/// on other platforms. This stub exists only so the workspace (and every
/// other, genuinely cross-platform crate in it) can be built and tested on
/// non-Windows hosts; it intentionally does not pretend to succeed.
#[cfg(not(windows))]
mod platform {
    use std::process::ExitCode;

    pub fn main() -> ExitCode {
        eprintln!("winbrew is a Windows package manager and does not run on this platform.");
        ExitCode::FAILURE
    }
}

fn main() -> ExitCode {
    platform::main()
}
