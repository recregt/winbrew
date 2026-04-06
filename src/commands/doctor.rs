use anyhow::Result;

use crate::{AppContext, services::doctor, ui::Ui};

pub fn run(ctx: &AppContext) -> Result<()> {
    let mut ui = Ui::new(ctx.ui);
    ui.page_title("System Health Check");
    ui.info("Inspecting environment...");
    let report = doctor::health_report(ctx)?;
    ui.display_key_values(&report.to_kv());
    ui.info("");

    ui.info("Loading installed packages...");
    let packages = doctor::installed_packages()?;
    ui.info(format!("Found {} package(s). Scanning...", packages.len()));

    let progress = ui.progress_bar();
    let broken = doctor::scan_packages_with_progress(&packages, &progress);
    progress.finish_and_clear();

    render_results(&mut ui, &broken);

    Ok(())
}

fn render_results<W: std::io::Write>(ui: &mut Ui<W>, broken: &[doctor::Diagnosis]) {
    if broken.is_empty() {
        ui.success("Your Winbrew installation is healthy!");
        return;
    }

    ui.notice(format!("Broken installs: {}", broken.len()));
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
