use anyhow::Result;
use comfy_table::{Cell, Color, Table, presets::UTF8_FULL_CONDENSED};
use dialoguer::{Confirm, theme::ColorfulTheme};
use indicatif::{ProgressBar, ProgressStyle};
use std::time::Duration;

use crate::database;
use crate::models::Package;

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
        if matches!(self.config_bool("default_yes")?, Some(true)) {
            return Ok(true);
        }

        Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(message)
            .default(default)
            .interact()
            .map_err(Into::into)
    }

    pub fn progress_bar(&self) -> ProgressBar {
        let pb = ProgressBar::new(0);

        let style = if self.color_enabled() {
            ProgressStyle::with_template(
                "{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})",
            )
        } else {
            ProgressStyle::with_template("{spinner} [{bar:40}] {bytes}/{total_bytes} ({eta})")
        }
        .unwrap()
        .progress_chars("█▉▊▋▌▍▎▏  ");

        pb.set_style(style);
        pb
    }

    pub fn spinner<T, F: FnOnce() -> T>(&self, message: impl Into<String>, f: F) -> T {
        let spinner = ProgressBar::new_spinner();

        let style = if self.color_enabled() {
            ProgressStyle::with_template(SPINNER_TEMPLATE)
                .unwrap()
                .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", ""])
        } else {
            ProgressStyle::with_template("{spinner} {msg}")
                .unwrap()
                .tick_strings(&["-", "\\", "|", "/", "-"])
        };

        spinner.set_style(style);
        spinner.set_message(message.into());
        spinner.enable_steady_tick(Duration::from_millis(80));

        let result = f();
        spinner.finish_and_clear();
        result
    }

    pub fn display_packages(&self, packages: &[Package]) {
        if packages.is_empty() {
            self.notice("No packages installed via winbrew.");
            return;
        }

        let color = self.color_enabled();
        let mut table = self.build_table([
            header_cell("Name", color, Color::Green),
            header_cell("Version", color, Color::Cyan),
            header_cell("Status", color, Color::DarkGrey),
            header_cell("Installed At", color, Color::DarkGrey),
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

    pub fn display_key_values(&self, rows: &[(String, String)]) {
        let color = self.color_enabled();
        let mut table = self.build_table([
            header_cell("Key", color, Color::Green),
            header_cell("Value", color, Color::Cyan),
        ]);

        for (key, value) in rows {
            table.add_row([Cell::new(key), Cell::new(value)]);
        }

        println!("{table}");
    }

    fn build_table(&self, headers: impl IntoIterator<Item = Cell>) -> Table {
        let mut table = Table::new();
        table.load_preset(UTF8_FULL_CONDENSED).set_header(headers);
        table
    }

    fn config_bool(&self, key: &str) -> Result<Option<bool>> {
        let conn = match database::lock_conn() {
            Ok(conn) => conn,
            Err(_) => return Ok(None),
        };

        database::config_bool(&conn, key)
    }

    fn color_enabled(&self) -> bool {
        self.config_bool("color").ok().flatten().unwrap_or(true)
    }
}

impl Default for Ui {
    fn default() -> Self {
        Self::new()
    }
}

fn header_cell(label: &str, color_enabled: bool, fg: Color) -> Cell {
    let cell = Cell::new(label);
    if color_enabled { cell.fg(fg) } else { cell }
}
