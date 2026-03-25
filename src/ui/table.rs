use super::Ui;
use super::theme::{header_cell, terminal_width};
use crate::models::{Package, PackageCandidate};
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

    pub fn display_candidates(&mut self, candidates: &[PackageCandidate]) {
        if candidates.is_empty() {
            self.notice("No matching packages found.");
            return;
        }

        let color = self.color_enabled;
        let mut table = self.build_table([
            header_cell("#", color, Color::DarkGrey),
            header_cell("Name", color, Color::Green),
            header_cell("Identifier", color, Color::DarkGrey),
            header_cell("Version", color, Color::Cyan),
            header_cell("Publisher", color, Color::DarkGrey),
        ]);

        for (index, candidate) in candidates.iter().enumerate() {
            let display_publisher = candidate
                .publisher
                .as_deref()
                .unwrap_or_else(|| candidate.identifier.split('.').next().unwrap_or("Unknown"));

            table.add_row([
                Cell::new(index + 1),
                Cell::new(
                    candidate
                        .package_name
                        .as_deref()
                        .unwrap_or(&candidate.identifier),
                ),
                Cell::new(&candidate.identifier),
                Cell::new(&candidate.version),
                Cell::new(display_publisher),
            ]);
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;
    use std::io::{Result as IoResult, Write};
    use std::rc::Rc;

    #[derive(Clone, Default)]
    struct BufferWriter(Rc<RefCell<Vec<u8>>>);

    impl BufferWriter {
        fn as_string(&self) -> String {
            String::from_utf8(self.0.borrow().clone()).expect("table output should be valid utf8")
        }
    }

    impl Write for BufferWriter {
        fn write(&mut self, buf: &[u8]) -> IoResult<usize> {
            self.0.borrow_mut().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> IoResult<()> {
            Ok(())
        }
    }

    #[test]
    fn display_candidates_renders_numbered_rows() {
        let writer = BufferWriter::default();
        let capture = writer.clone();
        let mut ui = Ui::with_writer(writer);
        let candidates = vec![PackageCandidate {
            identifier: "Microsoft.WindowsTerminal".to_string(),
            package_name: Some("Windows Terminal".to_string()),
            version: "1.21.2361.0".to_string(),
            description: Some("Terminal".to_string()),
            publisher: Some("Microsoft Corporation".to_string()),
            manifest_path: None,
        }];

        ui.display_candidates(&candidates);
        drop(ui);

        let output = capture.as_string();
        assert!(output.contains("Windows Terminal"));
        assert!(output.contains("Microsoft.WindowsTerminal"));
        assert!(output.contains('1'));
    }
}
