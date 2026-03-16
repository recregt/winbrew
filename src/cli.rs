use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::{cleaner, registry, scanner};

#[derive(Parser)]
#[command(
    name = "winbrew",
    about = "A modern package manager for Windows that installs, tracks, and cleanly removes software.",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// Installed applications list
    List {
        /// Optional application name filter (case-insensitive)
        filter: Option<String>,
    },
    /// Scan leftovers without deleting anything
    Scan {
        /// Application name to scan for
        name: String,
    },
    /// Delete scan matches (registry + directories)
    Clean {
        /// Application name to clean
        name: String,
        /// Show what would be deleted, do not change anything
        #[arg(long, help_heading = "Safety")]
        dry_run: bool,
        /// Skip confirmation prompt and delete immediately
        #[arg(long, conflicts_with = "dry_run", help_heading = "Safety")]
        yes: bool,
    },
}

impl Cli {
    pub fn run() -> Result<()> {
        Self::parse().command.run()
    }
}

impl Command {
    fn run(self) -> Result<()> {
        match self {
            Self::List { filter } => registry::show_installed_apps(filter.as_deref()),
            Self::Scan { name } => scanner::scan(&name),
            Self::Clean {
                name,
                dry_run,
                yes,
            } => cleaner::clean(&name, dry_run, yes),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command};
    use clap::Parser;

    #[test]
    fn parse_list_without_filter() {
        let cli = Cli::parse_from(["winbrew", "list"]);

        assert_eq!(cli.command, Command::List { filter: None });
    }

    #[test]
    fn parse_list_with_filter() {
        let cli = Cli::parse_from(["winbrew", "list", "steam"]);

        assert_eq!(
            cli.command,
            Command::List {
                filter: Some("steam".to_string())
            }
        );
    }

    #[test]
    fn parse_clean_with_dry_run() {
        let cli = Cli::parse_from(["winbrew", "clean", "steam", "--dry-run"]);

        assert_eq!(
            cli.command,
            Command::Clean {
                name: "steam".to_string(),
                dry_run: true,
                yes: false,
            }
        );
    }

    #[test]
    fn parse_clean_with_yes() {
        let cli = Cli::parse_from(["winbrew", "clean", "steam", "--yes"]);

        assert_eq!(
            cli.command,
            Command::Clean {
                name: "steam".to_string(),
                dry_run: false,
                yes: true,
            }
        );
    }

    #[test]
    fn reject_clean_yes_and_dry_run_together() {
        let result = Cli::try_parse_from(["winbrew", "clean", "steam", "--yes", "--dry-run"]);

        assert!(result.is_err());
    }
}
