use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "winbrew",
    version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("WINBREW_GIT_HASH"), ")"),
    about = "A modern package manager for Windows that tracks and cleanly removes software.",
    arg_required_else_help = true
)]
pub struct Cli {
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
    /// Use @winget/<id> or @scoop/<bucket>/<id> for exact package IDs.
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
    Doctor,

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
}
