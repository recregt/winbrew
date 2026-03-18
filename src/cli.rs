use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "winbrew",
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
    /// List packages installed by winbrew
    List,

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
    },
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command};
    use clap::Parser;

    #[test]
    fn parse_list() {
        let cli = Cli::parse_from(["winbrew", "list"]);

        assert_eq!(cli.command, Command::List);
    }

    #[test]
    fn parse_install_with_default_version() {
        let cli = Cli::parse_from(["winbrew", "install", "ripgrep"]);

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
        let cli = Cli::parse_from(["winbrew", "remove", "ripgrep", "--yes"]);

        assert_eq!(
            cli.command,
            Command::Remove {
                name: "ripgrep".to_string(),
                yes: true,
            }
        );
    }
}
