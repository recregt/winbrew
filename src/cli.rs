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
        #[arg(value_name = "QUERY", num_args = 1..)]
        query: Vec<String>,

        #[arg(long, short = 'v', value_name = "VERSION")]
        version: Option<String>,
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
    fn parse_install_with_single_word_query() {
        let cli = Cli::parse_from(["brew", "install", "ripgrep"]);

        assert_eq!(
            cli.command,
            Command::Install {
                query: vec!["ripgrep".to_string()],
                version: None,
            }
        );
    }

    #[test]
    fn parse_install_with_version_flag() {
        let cli = Cli::parse_from([
            "brew",
            "install",
            "Microsoft.WindowsTerminal",
            "--version",
            "1.9.1942.0",
        ]);

        assert_eq!(
            cli.command,
            Command::Install {
                query: vec!["Microsoft.WindowsTerminal".to_string()],
                version: Some("1.9.1942.0".to_string()),
            }
        );
    }

    #[test]
    fn parse_install_with_multi_word_query() {
        let cli = Cli::parse_from(["brew", "install", "windows", "terminal"]);

        assert_eq!(
            cli.command,
            Command::Install {
                query: vec!["windows".to_string(), "terminal".to_string()],
                version: None,
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

    #[test]
    fn parse_config_set_core_log_level() {
        let cli = Cli::parse_from(["brew", "config", "set", "core.log_level", "debug"]);

        assert_eq!(
            cli.command,
            Command::Config {
                command: super::ConfigCommand::Set {
                    key: "core.log_level".to_string(),
                    value: "debug".to_string(),
                },
            }
        );
    }

    #[test]
    fn parse_config_get_core_log_level() {
        let cli = Cli::parse_from(["brew", "config", "get", "core.log_level"]);

        assert_eq!(
            cli.command,
            Command::Config {
                command: super::ConfigCommand::Get {
                    key: "core.log_level".to_string(),
                },
            }
        );
    }
}
