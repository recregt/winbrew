#![allow(dead_code)]

use anyhow::Result;
use comfy_table::{Cell, Color, Table, presets::UTF8_FULL_CONDENSED};
use dialoguer::{Confirm, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

const SPINNER_TEMPLATE: &str = "{spinner:.green} {msg}";

pub struct Ui;

impl Ui {
    pub fn new() -> Self {
        Self
    }

    pub fn page_title(&self, title: &str) {
        println!("\n=== {title} ===");
    }

    pub fn info(&self, message: impl AsRef<str>) {
        println!("{}", message.as_ref());
    }

    pub fn success(&self, message: impl AsRef<str>) {
        println!("✓ {}", message.as_ref());
    }

    pub fn notice(&self, message: impl AsRef<str>) {
        println!("{}", message.as_ref());
    }

    pub fn confirm(&self, message: &str, default: bool) -> Result<bool> {
        Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(message)
            .default(default)
            .interact()
            .map_err(Into::into)
    }

    pub fn progress_bar(&self) -> ProgressBar {
        let pb = ProgressBar::new(0);
        pb.set_style(
            ProgressStyle::with_template(
                "{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
            )
            .unwrap()
            .progress_chars("█▉▊▋▌▍▎▏  "),
        );
        pb
    }

    pub fn spinner<T, F: FnOnce() -> T>(&self, message: impl Into<String>, f: F) -> T {
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

    pub fn display_packages(&self, packages: &[crate::database::Package]) {
        if packages.is_empty() {
            self.notice("No packages installed via winbrew.");
            return;
        }

        let mut table = self.build_table([
            Cell::new("Name").fg(Color::Green),
            Cell::new("Version").fg(Color::Cyan),
            Cell::new("Status").fg(Color::DarkGrey),
            Cell::new("Installed At").fg(Color::DarkGrey),
        ]);

        for pkg in packages {
            table.add_row([
                Cell::new(&pkg.name),
                Cell::new(&pkg.version),
                Cell::new(pkg.status.to_string()),
                Cell::new(&pkg.installed_at),
            ]);
        }

        println!("{table}");
    }

    fn build_table(&self, headers: impl IntoIterator<Item = Cell>) -> Table {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL_CONDENSED).set_header(headers);
        table
    }
}
