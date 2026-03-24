use anyhow::Result;

use crate::{database, services::doctor, ui::Ui};

pub fn run() -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("System Health Check");
    ui.info("Inspecting environment...");
    let report = database::get_health_report()?;
    ui.display_key_values(&report.to_kv());
    ui.info("");

    ui.info("Loading installed packages...");
    let conn = database::lock_conn()?;
    let packages = database::list_packages(&conn)?;
    ui.info(format!("Found {} package(s). Scanning...", packages.len()));

    let progress = ui.progress_bar();
    let broken = doctor::scan_packages_with_progress(&packages, &progress);
    progress.finish_and_clear();

    render_results(&mut ui, broken);

    Ok(())
}

fn render_results<W: std::io::Write>(ui: &mut Ui<W>, broken: Vec<doctor::Diagnosis>) {
    ui.notice(format!("Broken installs: {}", broken.len()));

    if broken.is_empty() {
        ui.success("Your Winbrew installation is healthy!");
    } else {
        ui.warn("Found the following issues:");
        for entry in broken {
            ui.notice(format!(
                "  - {} -> {} ({})",
                entry.package_name, entry.install_dir, entry.issue
            ));
        }
        ui.info("");
        ui.info("Suggestion: Try running 'winbrew repair' or reinstalling the affected packages.");
    }
}
