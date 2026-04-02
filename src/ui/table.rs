use super::Ui;
use super::theme::{header_cell, terminal_width};
use crate::models::CatalogPackage;
use crate::models::Package;
use comfy_table::{Cell, Color, Table, presets::UTF8_FULL_CONDENSED};
use std::io::Write;

impl<W: Write> Ui<W> {
    fn render_table(&mut self, table: Table) {
        let _ = writeln!(self.out, "{table}");
        let _ = self.out.flush();
    }

    pub fn display_packages(&mut self, packages: &[Package]) {
        if packages.is_empty() {
            self.notice("No packages installed via winbrew.");
            return;
        }

        let color = self.color_enabled;
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
                Cell::new(pkg.status.as_str()),
                Cell::new(&pkg.installed_at),
            ]);
        }

        self.render_table(table);
    }

    pub fn display_catalog_packages(&mut self, packages: &[CatalogPackage]) {
        if packages.is_empty() {
            self.notice("No catalog packages found.");
            return;
        }

        let color = self.color_enabled;
        let mut table = self.build_table([
            header_cell("Name", color, Color::Green),
            header_cell("Version", color, Color::Cyan),
            header_cell("Source", color, Color::DarkGrey),
            header_cell("Description", color, Color::DarkGrey),
        ]);

        for pkg in packages {
            table.add_row([
                Cell::new(&pkg.name),
                Cell::new(&pkg.version),
                Cell::new(&pkg.source),
                Cell::new(pkg.description.as_deref().unwrap_or("")),
            ]);
        }

        self.render_table(table);
    }

    pub fn display_key_values(&mut self, rows: &[(String, String)]) {
        let color = self.color_enabled;
        let mut table = self.build_table([
            header_cell("Key", color, Color::Green),
            header_cell("Value", color, Color::Cyan),
        ]);

        for (key, value) in rows {
            table.add_row([Cell::new(key), Cell::new(value)]);
        }

        self.render_table(table);
    }

    fn build_table(&self, headers: impl IntoIterator<Item = Cell>) -> Table {
        let mut table = Table::new();
        table
            .load_preset(UTF8_FULL_CONDENSED)
            .set_header(headers)
            .set_width(terminal_width());
        table
    }
}
