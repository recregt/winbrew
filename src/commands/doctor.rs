use anyhow::Result;

use crate::{
    core::{paths, shim},
    database,
    ui::Ui,
};

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
    ui.notice(format!("Bin dir: {}", paths_config.bin.to_string_lossy()));
    ui.notice(format!(
        "Packages dir: {}",
        paths_config.packages.to_string_lossy()
    ));

    ui.info("Loading installed packages...");
    let packages = database::list_packages(&conn)?;
    ui.info(format!("Loaded {} package(s).", packages.len()));
    ui.notice(format!("Installed packages: {}", packages.len()));

    let mut broken = Vec::new();
    ui.info("Scanning installed shims...");
    for (index, pkg) in packages.iter().enumerate() {
        if index % 25 == 0 {
            ui.info(format!(
                "Scanning package {}/{}...",
                index + 1,
                packages.len()
            ));
        }

        for shim_entry in &pkg.shims {
            if !shim::exists_at(&install_root, &shim_entry.name) {
                broken.push(format!("{} -> {}", pkg.name, shim_entry.name));
                continue;
            }

            match shim::read_at(&install_root, &shim_entry.name) {
                Ok((path, _)) => {
                    if path.is_empty() {
                        broken.push(format!(
                            "{} -> {} (empty target)",
                            pkg.name, shim_entry.name
                        ));
                    }
                }
                Err(_) => broken.push(format!("{} -> {} (unreadable)", pkg.name, shim_entry.name)),
            }
        }
    }

    ui.notice(format!("Broken shims: {}", broken.len()));

    if broken.is_empty() {
        ui.success("No broken shims found.");
    } else {
        ui.notice("Broken shims:");
        for entry in broken {
            ui.notice(format!("  {entry}"));
        }
        ui.notice("Health check completed with issues.");
    }

    Ok(())
}
