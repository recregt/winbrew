use super::Ui;
use super::theme::{header_cell, terminal_width};
use comfy_table::{Cell, Color, ContentArrangement, Row, Table, presets::UTF8_FULL_CONDENSED};
use std::io::Write;
use winbrew_models::catalog::package::CatalogPackage;
use winbrew_models::install::installed::{InstalledPackage, PackageStatus};

impl<W: Write> Ui<W> {
    fn render_table(&mut self, table: Table) {
        let _ = writeln!(self.out, "{table}");
        let _ = self.out.flush();
    }

    pub fn display_packages(&mut self, packages: &[InstalledPackage]) {
        if packages.is_empty() {
            self.notice("No packages installed via winbrew.");
            return;
        }

        self.render_section_label("installed packages");

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
                status_badge(pkg.status, color),
                Cell::new(&pkg.installed_at).fg(Color::DarkGrey),
            ]);
        }

        self.render_table(table);
    }

    pub fn display_catalog_packages(&mut self, packages: &[CatalogPackage]) {
        if packages.is_empty() {
            self.notice("No catalog packages found.");
            return;
        }

        self.render_section_label("catalog packages");

        let color = self.color_enabled;
        let mut table = self.build_table([
            header_cell("Name", color, Color::Green),
            header_cell("Version", color, Color::Cyan),
            header_cell("Source", color, Color::Magenta),
            header_cell("Package ID", color, Color::DarkGrey),
        ]);
        table.set_content_arrangement(ContentArrangement::Dynamic);

        for pkg in packages {
            let mut row = Row::new();
            row.add_cell(Cell::new(&pkg.name));
            row.add_cell(Cell::new(pkg.version.to_string()));
            row.add_cell(source_cell(pkg.source, color));
            row.add_cell(Cell::new(pkg.id.as_str()));
            row.max_height(1);
            table.add_row(row);
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
            .set_truncation_indicator("…")
            .set_header(headers)
            .set_width(terminal_width());
        table
    }

    fn render_section_label(&mut self, label: &str) {
        if self.color_enabled {
            let _ = writeln!(self.out, "\x1b[2;37m{label}\x1b[0m");
        } else {
            let _ = writeln!(self.out, "{label}");
        }
    }
}

fn status_badge(status: PackageStatus, color_enabled: bool) -> Cell {
    let (label, color) = match status {
        PackageStatus::Installing => ("[installing]", Color::DarkGrey),
        PackageStatus::Ok => ("[installed]", Color::Green),
        PackageStatus::Updating => ("[update available]", Color::Yellow),
        PackageStatus::Failed => ("[broken]", Color::Red),
    };

    let cell = Cell::new(label);
    if color_enabled { cell.fg(color) } else { cell }
}

fn source_cell(source: impl AsRef<str>, color_enabled: bool) -> Cell {
    let source = source.as_ref();
    let color = match source.to_ascii_lowercase().as_str() {
        "winget" => Color::Blue,
        "scoop" => Color::Yellow,
        _ => Color::DarkGrey,
    };

    let cell = Cell::new(source);
    if color_enabled { cell.fg(color) } else { cell }
}

#[cfg(test)]
mod tests {
    use crate::{Ui, UiSettings};
    use std::io::{Result as IoResult, Write};
    use std::sync::{Arc, Mutex};
    use winbrew_models::domains::catalog::CatalogPackage;
    use winbrew_models::domains::install::{EngineKind, InstallerType};
    use winbrew_models::domains::installed::{InstalledPackage, PackageStatus};
    use winbrew_models::domains::package::PackageSource;
    use winbrew_models::domains::shared::Version;

    struct SharedBuffer {
        bytes: Arc<Mutex<Vec<u8>>>,
    }

    impl SharedBuffer {
        fn new(bytes: Arc<Mutex<Vec<u8>>>) -> Self {
            Self { bytes }
        }
    }

    impl Write for SharedBuffer {
        fn write(&mut self, buffer: &[u8]) -> IoResult<usize> {
            let mut bytes = self.bytes.lock().expect("buffer lock should be available");
            bytes.extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> IoResult<()> {
            Ok(())
        }
    }

    fn catalog_package(description: Option<&str>) -> CatalogPackage {
        CatalogPackage {
            id: "scoop/main/Contoso.App".into(),
            name: "Contoso App".to_string(),
            version: Version::parse("1.2.3").expect("version should parse"),
            source: PackageSource::Scoop,
            description: description.map(ToOwned::to_owned),
            homepage: None,
            license: None,
            publisher: None,
        }
    }

    fn installed_package(status: PackageStatus) -> InstalledPackage {
        InstalledPackage {
            name: "Contoso App".to_string(),
            version: "1.2.3".to_string(),
            kind: InstallerType::Portable,
            deployment_kind: InstallerType::Portable.deployment_kind(),
            engine_kind: EngineKind::Portable,
            engine_metadata: None,
            install_dir: "C:\\Apps\\Contoso".to_string(),
            dependencies: Vec::new(),
            status,
            installed_at: "2026-04-07T12:00:00Z".to_string(),
        }
    }

    #[test]
    fn display_catalog_packages_hides_descriptions() {
        let shared_bytes = Arc::new(Mutex::new(Vec::new()));
        let writer = SharedBuffer::new(Arc::clone(&shared_bytes));
        let mut ui = Ui::with_writer(
            writer,
            UiSettings {
                color_enabled: false,
                default_yes: false,
            },
        );

        let long_description = "This is a very long Scoop description that should be truncated so that it does not dominate the search table layout or wrap the row unexpectedly.";

        ui.display_catalog_packages(&[catalog_package(Some(long_description))]);
        ui.display_catalog_packages(&[
            catalog_package(Some("Short description")),
            CatalogPackage {
                id: "winget/main/Fabrikam.Tool".into(),
                name: "Fabrikam Tool".to_string(),
                version: Version::parse("2.0.0").expect("version should parse"),
                source: PackageSource::Winget,
                description: None,
                homepage: None,
                license: None,
                publisher: None,
            },
        ]);
        ui.display_catalog_packages(&[catalog_package(None)]);

        let output = String::from_utf8(
            shared_bytes
                .lock()
                .expect("buffer lock should be available")
                .clone(),
        )
        .expect("rendered output should be valid UTF-8");

        assert!(!output.contains(long_description));
        assert!(!output.contains("No description available"));
        assert!(output.contains("Contoso App"));
        assert!(output.contains("winget/main/Fabrikam.Tool"));
        assert!(output.contains("scoop/main/Contoso.App"));
        assert!(output.contains("catalog packages"));
    }

    #[test]
    fn display_packages_renders_status_badges_and_section_label() {
        let shared_bytes = Arc::new(Mutex::new(Vec::new()));
        let writer = SharedBuffer::new(Arc::clone(&shared_bytes));
        let mut ui = Ui::with_writer(
            writer,
            UiSettings {
                color_enabled: false,
                default_yes: false,
            },
        );

        ui.display_packages(&[
            installed_package(PackageStatus::Ok),
            installed_package(PackageStatus::Updating),
            installed_package(PackageStatus::Failed),
            installed_package(PackageStatus::Installing),
        ]);

        let output = String::from_utf8(
            shared_bytes
                .lock()
                .expect("buffer lock should be available")
                .clone(),
        )
        .expect("rendered output should be valid UTF-8");

        assert!(output.contains("installed packages"));
        assert!(output.contains("[installed]"));
        assert!(output.contains("[update available]"));
        assert!(output.contains("[broken]"));
        assert!(output.contains("[installing]"));
    }
}
