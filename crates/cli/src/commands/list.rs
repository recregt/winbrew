//! List command wrapper for installed-package views.
//!
//! The wrapper handles empty-state messaging and the final installed-package
//! total while the app layer provides the filtered package data.

use anyhow::Result;
use std::io::Write;

use crate::{CommandContext, app::list};
use winbrew_ui::Ui;

pub fn run(ctx: &CommandContext, query: &[String]) -> Result<()> {
    let mut ui = ctx.ui();
    run_with_ui(&mut ui, query)
}

fn run_with_ui<W: Write>(ui: &mut Ui<W>, query: &[String]) -> Result<()> {
    ui.page_title("Installed Packages");

    let query_text = (!query.is_empty()).then(|| query.join(" "));

    let packages = list::list_packages(query_text.as_deref())?;

    if packages.is_empty() {
        match query_text {
            Some(q) => ui.notice(format!("No installed packages matching '{q}'.")),
            None => ui.notice("No packages are currently installed."),
        }

        return Ok(());
    }

    ui.display_packages(&packages);
    ui.info(format!("\nTotal: {} package(s) installed.", packages.len()));

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run_with_ui;
    use crate::commands::test_support::{buffer_text, buffered_ui};
    use crate::database::{self, Config};
    use crate::models::domains::install::{EngineKind, InstallerType};
    use crate::models::domains::installed::{InstalledPackage, PackageStatus};
    use tempfile::tempdir;
    use winbrew_ui::UiSettings;

    fn sample_package() -> InstalledPackage {
        InstalledPackage {
            name: "Contoso App".to_string(),
            version: "1.2.3".to_string(),
            kind: InstallerType::Portable,
            deployment_kind: InstallerType::Portable.deployment_kind(),
            engine_kind: EngineKind::Portable,
            engine_metadata: None,
            install_dir: r"C:\Apps\Contoso".to_string(),
            dependencies: Vec::new(),
            status: PackageStatus::Ok,
            installed_at: "2026-04-07T12:00:00Z".to_string(),
        }
    }

    #[test]
    fn run_with_ui_reports_query_specific_empty_state() {
        let temp_dir = tempdir().expect("temp dir");
        let config = Config::load_at(temp_dir.path()).expect("config should load");
        database::init(&config.resolved_paths()).expect("database should initialize");

        let (mut ui, out, err) = buffered_ui(UiSettings::default());
        let query = ["Contoso".to_string(), "App".to_string()];

        run_with_ui(&mut ui, &query).expect("list should succeed");

        assert!(buffer_text(&out).trim().is_empty());
        assert!(buffer_text(&err).contains("No installed packages matching 'Contoso App'."));
    }

    #[test]
    fn run_with_ui_renders_installed_packages() {
        let temp_dir = tempdir().expect("temp dir");
        let config = Config::load_at(temp_dir.path()).expect("config should load");
        database::init(&config.resolved_paths()).expect("database should initialize");

        {
            let conn = database::get_conn().expect("database connection");
            database::insert_package(&conn, &sample_package()).expect("package should seed");
        }

        let (mut ui, out, err) = buffered_ui(UiSettings::default());

        run_with_ui(&mut ui, &[]).expect("list should succeed");

        let out = buffer_text(&out);
        let err = buffer_text(&err);

        assert!(out.contains("installed packages"));
        assert!(out.contains("Contoso App"));
        assert!(out.contains("1.2.3"));
        assert!(err.contains("Total: 1 package(s) installed."));
    }
}
