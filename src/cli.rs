use anyhow::Result;
use clap::{Parser, Subcommand};
use comfy_table::{Cell, Color, Table, presets::UTF8_FULL_CONDENSED};
use dialoguer::{Confirm, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use crate::{cleaner, registry, scanner};

const SPINNER_TEMPLATE: &str = "{spinner:.green} {msg}";

#[derive(Parser)]
#[command(
    name = "winbrew",
    version,
    about = "A modern package manager for Windows that installs, tracks, and cleanly removes software.",
    arg_required_else_help = true
)]
pub struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, PartialEq, Eq, Subcommand)]
pub enum Command {
    /// List installed applications
    List {
        #[arg(value_name = "FILTER")]
        filter: Option<String>,
    },

    /// Scan for application leftovers without deleting anything
    Scan {
        #[arg(value_name = "APP_NAME")]
        name: String,
    },

    /// Remove application leftovers (registry and directories)
    Clean {
        #[arg(value_name = "APP_NAME")]
        name: String,

        #[arg(long, short = 'd', help_heading = "Safety")]
        dry_run: bool,

        #[arg(long, short = 'y', conflicts_with = "dry_run", help_heading = "Safety")]
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
            Self::List { filter } => {
                let apps = registry::collect_installed_apps(filter.as_deref())?;
                display_installed_apps(&apps);
                Ok(())
            }
            Self::Scan { name } => {
                let scan_result =
                    with_spinner(format!("Scanning '{name}'..."), || scanner::collect(&name))?;
                display_scan_result(&scan_result);
                Ok(())
            }
            Self::Clean { name, dry_run, yes } => {
                // 1. Collect data
                let scan_result =
                    with_spinner(format!("Scanning '{name}'..."), || scanner::collect(&name))?;

                if scan_result.registry_matches.is_empty()
                    && scan_result.directory_matches.is_empty()
                {
                    println!("Nothing to clean for '{name}'.");
                    return Ok(());
                }

                // 2. Show findings
                display_scan_result(&scan_result);

                if dry_run {
                    println!("\n[Dry Run] No changes were made.");
                    return Ok(());
                }

                // 2.1 Request User Confirmation
                if !yes {
                    let confirmed = Confirm::with_theme(&ColorfulTheme::default())
                        .with_prompt("Do you want to permanently delete these items?")
                        .default(false)
                        .interact()?;

                    if !confirmed {
                        println!("\nAborted.");
                        return Ok(());
                    }
                }

                // 3. Trigger Deletion Operation
                let report = with_spinner("Cleaning matched items...", || {
                    cleaner::execute_clean(&scan_result)
                });

                // 4. Display Results
                println!("\nSuccessfully deleted {} item(s).", report.success_count);

                if !report.failures.is_empty() {
                    let mut failures_table = build_table([Cell::new("Failed").fg(Color::Red)]);
                    for f in &report.failures {
                        failures_table.add_row([Cell::new(f)]);
                    }

                    eprintln!("\nFailures:\n{failures_table}");
                    anyhow::bail!("Clean finished with {} failure(s).", report.failures.len());
                }

                Ok(())
            }
        }
    }
}

fn with_spinner<T, F: FnOnce() -> T>(message: impl Into<String>, f: F) -> T {
    let spinner = ProgressBar::new_spinner();
    spinner.set_style(
        ProgressStyle::with_template(SPINNER_TEMPLATE)
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", ""]),
    );
    spinner.set_message(message.into());
    spinner.enable_steady_tick(Duration::from_millis(80));

    let result = f();
    spinner.finish_and_clear();
    result
}

fn build_table(headers: impl IntoIterator<Item = Cell>) -> Table {
    let mut table = Table::new();
    table.load_preset(UTF8_FULL_CONDENSED).set_header(headers);
    table
}

fn display_installed_apps(apps: &[registry::AppInfo]) {
    if apps.is_empty() {
        println!("No matching applications found.");
        return;
    }

    let mut table = build_table([
        Cell::new("Name").fg(Color::Green),
        Cell::new("Version").fg(Color::Cyan),
        Cell::new("Publisher").fg(Color::DarkGrey),
    ]);

    for app in apps {
        table.add_row([
            Cell::new(&app.name),
            Cell::new(&app.version),
            Cell::new(&app.publisher),
        ]);
    }

    println!("{table}");
}

fn display_scan_result(result: &scanner::ScanResult) {
    if result.registry_matches.is_empty() {
        println!("[Registry]\n  (nothing found)");
    } else {
        let mut registry_table = build_table([
            Cell::new("Hive").fg(Color::Green),
            Cell::new("Key").fg(Color::Cyan),
            Cell::new("Display Name").fg(Color::DarkGrey),
        ]);

        for m in &result.registry_matches {
            registry_table.add_row([
                Cell::new(m.root_label),
                Cell::new(&m.key_name),
                Cell::new(&m.display_name),
            ]);
        }

        println!("[Registry]\n{registry_table}");
    }

    if result.directory_matches.is_empty() {
        println!("\n[Directories]\n  (nothing found)");
    } else {
        let mut directories_table = build_table([Cell::new("Directories").fg(Color::Green)]);

        for d in &result.directory_matches {
            directories_table.add_row([Cell::new(d.display().to_string())]);
        }

        println!("\n[Directories]\n{directories_table}");
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
    fn parse_scan_with_name() {
        let cli = Cli::parse_from(["winbrew", "scan", "steam"]);

        assert_eq!(
            cli.command,
            Command::Scan {
                name: "steam".to_string(),
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
