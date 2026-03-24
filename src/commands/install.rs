use anyhow::{Context, Result};

use crate::{models::PackageCandidate, services::install, ui::Ui};

pub fn run(query: &[String], version: Option<&str>) -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("Install Package");

    ui.info("Resolving package manifest...");
    let resolved = match install::resolve::resolve(query, version)? {
        install::Resolution::Resolved(resolved) => resolved,
        install::Resolution::Candidates(candidates) => {
            handle_selection(&mut ui, query, &candidates, version)?
        }
    };

    let plan = install::resolve_plan(&resolved.identifier, &resolved.version)
        .context("failed to generate installation plan")?;
    ui.info(format!(
        "Package found: {}@{}",
        plan.name, plan.package_version
    ));

    let pb = ui.progress_bar();
    let result = install::execute_plan(&plan, |event| match event {
        install::Progress::Downloading { current, total } => {
            if total == 0 {
                pb.set_message(format!("Downloading {} bytes...", current));
            } else {
                pb.set_length(total);
                pb.set_position(current);
            }
        }
        install::Progress::Status(msg) => pb.set_message(msg),
    });

    pb.finish_and_clear();
    result?;

    ui.success(format!(
        "Successfully installed {} v{}",
        plan.name, plan.package_version
    ));

    Ok(())
}

fn handle_selection<W: std::io::Write>(
    ui: &mut Ui<W>,
    query: &[String],
    candidates: &[PackageCandidate],
    requested_version: Option<&str>,
) -> Result<install::ResolvedInstall> {
    ui.notice(format!(
        "Multiple packages matched '{}'. Select one by number:",
        query.join(" ")
    ));
    ui.display_candidates(candidates);

    let choice = ui.prompt_number("Choose package", candidates.len())?;
    let candidate = &candidates[choice];

    Ok(install::ResolvedInstall {
        identifier: candidate.identifier.clone(),
        version: requested_version
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| candidate.version.clone()),
    })
}
