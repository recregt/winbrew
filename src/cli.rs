use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "brew",
    version = concat!(env!("CARGO_PKG_VERSION"), " (", env!("WINBREW_GIT_HASH"), ")"),
    about = "A modern package manager for Windows that installs, tracks, and cleanly removes software.",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// List packages installed by brew
    List,

    /// Show effective runtime settings and paths
    Info,

    /// Check local brew installation health
    Doctor,

    /// Install a package from the configured package repository
    Install {
        #[arg(value_name = "PACKAGE")]
        name: String,

        #[arg(value_name = "VERSION", default_value = "latest")]
        version: String,
    },

    /// Remove a package and its tracked files
    Remove {
        #[arg(value_name = "PACKAGE")]
        name: String,

        #[arg(long, short = 'y', help_heading = "Safety")]
        yes: bool,

        #[arg(long, help_heading = "Safety")]
        force: bool,
    },

    /// Get or set brew configuration values
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
        value: String,
    },
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command};
    use clap::Parser;

    #[test]
    fn parse_list() {
        let cli = Cli::parse_from(["brew", "list"]);

        assert_eq!(cli.command, Command::List);
    }

    #[test]
    fn parse_info() {
        let cli = Cli::parse_from(["brew", "info"]);

        assert_eq!(cli.command, Command::Info);
    }

    #[test]
    fn parse_doctor() {
        let cli = Cli::parse_from(["brew", "doctor"]);

        assert_eq!(cli.command, Command::Doctor);
    }

    #[test]
    fn parse_install_with_default_version() {
        let cli = Cli::parse_from(["brew", "install", "ripgrep"]);

        assert_eq!(
            cli.command,
            Command::Install {
                name: "ripgrep".to_string(),
                version: "latest".to_string(),
            }
        );
    }

    #[test]
    fn parse_remove_with_yes() {
        let cli = Cli::parse_from(["brew", "remove", "ripgrep", "--yes"]);

        assert_eq!(
            cli.command,
            Command::Remove {
                name: "ripgrep".to_string(),
                yes: true,
                force: false,
            }
        );
    }

    #[test]
    fn parse_remove_with_force() {
        let cli = Cli::parse_from(["brew", "remove", "ripgrep", "--force"]);

        assert_eq!(
            cli.command,
            Command::Remove {
                name: "ripgrep".to_string(),
                yes: false,
                force: true,
            }
        );
    }

    #[test]
    fn parse_config_list() {
        let cli = Cli::parse_from(["brew", "config", "list"]);

        assert_eq!(
            cli.command,
            Command::Config {
                command: super::ConfigCommand::List,
            }
        );
    }
}
