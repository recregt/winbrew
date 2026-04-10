use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "winbrew",
    version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("WINBREW_GIT_HASH"), ")"),
    about = "A modern package manager for Windows that tracks and cleanly removes software.",
    arg_required_else_help = true
)]
pub struct Cli {
    /// Increase error detail output.
    #[arg(short, long, global = true, action = clap::ArgAction::Count, help_heading = "Output")]
    pub verbose: u8,

    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// List packages installed by winbrew
    List {
        #[arg(value_name = "QUERY", num_args = 0..)]
        query: Vec<String>,
    },

    /// Search the package catalog
    Search {
        #[arg(value_name = "QUERY", num_args = 1..)]
        query: Vec<String>,
    },

    /// Install a package from the catalog
    /// Use `@winget/<id>` or `@scoop/<bucket>/<id>` for exact package IDs.
    Install {
        #[arg(value_name = "QUERY", num_args = 1..)]
        query: Vec<String>,

        #[arg(long, help_heading = "Safety")]
        ignore_checksum_security: bool,
    },

    /// Show effective runtime settings and paths
    Info,

    /// Print the winbrew version
    Version,

    /// Check local winbrew installation health
    Doctor {
        /// Emit machine-readable JSON instead of the standard UI output
        #[arg(long, help_heading = "Output")]
        json: bool,

        /// Treat warnings as failures and exit non-zero when warnings are found
        #[arg(long, help_heading = "Output")]
        warn_as_error: bool,
    },

    /// Refresh the package catalog
    Update,

    /// Remove a package and its tracked files
    Remove {
        #[arg(value_name = "PACKAGE")]
        name: String,

        #[arg(long, short = 'y', help_heading = "Safety")]
        yes: bool,

        #[arg(long, help_heading = "Safety")]
        force: bool,
    },

    /// Get or set winbrew configuration values
    Config {
        #[command(subcommand)]
        command: ConfigCommand,
    },
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum ConfigCommand {
    /// List all configuration values
    List,

    /// Read a configuration value
    Get {
        #[arg(value_name = "KEY")]
        key: String,
    },

    /// Store a configuration value
    Set {
        #[arg(value_name = "KEY")]
        key: String,

        #[arg(value_name = "VALUE")]
        value: Option<String>,
    },

    /// Remove a configuration value
    Unset {
        #[arg(value_name = "KEY")]
        key: String,
    },
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, ConfigCommand};
    use clap::Parser;

    #[test]
    fn parse_verbose_doctor() {
        let cli = Cli::parse_from(["brew", "doctor", "-vv"]);

        assert_eq!(cli.verbose, 2);
        assert_eq!(
            cli.command,
            Command::Doctor {
                json: false,
                warn_as_error: false,
            }
        );
    }

    #[test]
    fn parse_config_unset_core_log_level() {
        let cli = Cli::parse_from(["brew", "config", "unset", "core.log_level"]);

        assert_eq!(
            cli.command,
            Command::Config {
                command: ConfigCommand::Unset {
                    key: "core.log_level".to_string(),
                },
            }
        );
    }

    #[test]
    fn parse_doctor() {
        let cli = Cli::parse_from(["brew", "doctor"]);

        assert_eq!(
            cli.command,
            Command::Doctor {
                json: false,
                warn_as_error: false,
            }
        );
    }

    #[test]
    fn parse_doctor_json() {
        let cli = Cli::parse_from(["brew", "doctor", "--json"]);

        assert_eq!(
            cli.command,
            Command::Doctor {
                json: true,
                warn_as_error: false,
            }
        );
    }

    #[test]
    fn parse_doctor_warn_as_error() {
        let cli = Cli::parse_from(["brew", "doctor", "--warn-as-error"]);

        assert_eq!(
            cli.command,
            Command::Doctor {
                json: false,
                warn_as_error: true,
            }
        );
    }
}
