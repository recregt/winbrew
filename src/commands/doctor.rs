use anyhow::Result;

use crate::{
    core::{paths, shim},
    database,
    ui::Ui,
};

pub fn run() -> Result<()> {
    let ui = Ui::new();
    ui.page_title("Doctor");

    let conn = database::lock_conn()?;
    let install_root_value = database::config_string(&conn, "install_dir")?;
    let install_root = paths::install_root(install_root_value.as_deref());

    let mut rows = Vec::new();
    rows.push(("Database reachable".to_string(), "yes".to_string()));
    rows.push((
        "Database".to_string(),
        paths::db_path().to_string_lossy().to_string(),
    ));
    rows.push((
        "Database exists".to_string(),
        if paths::db_path().exists() {
            "yes"
        } else {
            "no"
        }
        .to_string(),
    ));
    rows.push((
        "Install root source".to_string(),
        if install_root_value.is_some() {
            "config:install_dir".to_string()
        } else {
            "default".to_string()
        },
    ));
    rows.push((
        "Install root".to_string(),
        install_root.to_string_lossy().to_string(),
    ));
    rows.push((
        "Install root exists".to_string(),
        if install_root.exists() { "yes" } else { "no" }.to_string(),
    ));
    rows.push((
        "Bin dir".to_string(),
        paths::bin_dir_at(&install_root)
            .to_string_lossy()
            .to_string(),
    ));
    rows.push((
        "Packages dir".to_string(),
        paths::packages_dir_at(&install_root)
            .to_string_lossy()
            .to_string(),
    ));

    let packages = database::list_packages(&conn)?;
    rows.push(("Installed packages".to_string(), packages.len().to_string()));

    let mut broken = Vec::new();
    for pkg in &packages {
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

    rows.push(("Broken shims".to_string(), broken.len().to_string()));

    ui.display_key_values(&rows);

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
