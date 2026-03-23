use anyhow::Result;
use std::path::Path;

use crate::{core::paths, database, ui::Ui};

pub fn run() -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("Doctor");
    ui.info("Checking database...");

    let conn = database::lock_conn()?;
    let config = database::Config::current();
    let paths_config = config.resolved_paths();
    let install_root = paths_config.root.clone();

    ui.notice("Database reachable: yes");
    ui.notice(format!("Database: {}", paths::db_path().to_string_lossy()));
    ui.notice(format!(
        "Database exists: {}",
        if paths::db_path().exists() {
            "yes"
        } else {
            "no"
        }
    ));
    ui.notice("Install root source: config:paths.root");
    ui.notice(format!("Install root: {}", install_root.to_string_lossy()));
    ui.notice(format!(
        "Install root exists: {}",
        if install_root.exists() { "yes" } else { "no" }
    ));
    ui.notice(format!(
        "Packages dir: {}",
        paths_config.packages.to_string_lossy()
    ));

    ui.info("Loading installed packages...");
    let packages = database::list_packages(&conn)?;
    ui.info(format!("Loaded {} package(s).", packages.len()));
    ui.notice(format!("Installed packages: {}", packages.len()));

    let mut broken = Vec::new();
    ui.info("Scanning installed packages...");
    for (index, pkg) in packages.iter().enumerate() {
        if index % 25 == 0 {
            ui.info(format!(
                "Scanning package {}/{}...",
                index + 1,
                packages.len()
            ));
        }

        let install_dir = Path::new(&pkg.install_dir);
        if !install_dir.exists() {
            broken.push(format!(
                "{} -> {} (missing install directory)",
                pkg.name, pkg.install_dir
            ));
            continue;
        }

        if !install_dir.is_dir() {
            broken.push(format!(
                "{} -> {} (not a directory)",
                pkg.name, pkg.install_dir
            ));
            continue;
        }

        if std::fs::read_dir(install_dir).is_err() {
            broken.push(format!("{} -> {} (unreadable)", pkg.name, pkg.install_dir));
        }
    }

    ui.notice(format!("Broken installs: {}", broken.len()));

    if broken.is_empty() {
        ui.success("No broken installs found.");
    } else {
        ui.notice("Broken installs:");
        for entry in broken {
            ui.notice(format!("  {entry}"));
        }
        ui.notice("Health check completed with issues.");
    }

    Ok(())
}
