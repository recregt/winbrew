use anyhow::{Context, Result, bail};

use crate::{models::PackageCandidate, services::install, ui::Ui};

pub fn run(query: &[String], version: Option<&str>, select: Option<usize>) -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("Install Package");

    ui.info("Resolving package manifest...");
    let resolved = match install::resolve::resolve(query, version)? {
        install::Resolution::Resolved(resolved) => resolved,
        install::Resolution::Candidates(candidates) => {
            handle_selection(&mut ui, query, &candidates, version, select)?
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
    select: Option<usize>,
) -> Result<install::ResolvedInstall> {
    if let Some(choice) = select {
        if choice == 0 || choice > candidates.len() {
            bail!(
                "install selection must be between 1 and {}",
                candidates.len()
            );
        }

        let candidate = &candidates[choice - 1];
        return Ok(resolved_install(candidate, requested_version));
    }

    ui.notice(format!(
        "Multiple packages matched '{}'. Select one by number:",
        query.join(" ")
    ));
    ui.display_candidates(candidates);

    let choice = ui.prompt_number("Choose package", candidates.len())?;
    let candidate = &candidates[choice];

    Ok(resolved_install(candidate, requested_version))
}

fn resolved_install(
    candidate: &PackageCandidate,
    requested_version: Option<&str>,
) -> install::ResolvedInstall {
    install::ResolvedInstall {
        identifier: candidate.identifier.clone(),
        version: requested_version
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| candidate.version.clone()),
    }
}

#[cfg(test)]
mod tests {
    use super::{handle_selection, resolved_install};
    use crate::models::PackageCandidate;
    use crate::ui::Ui;

    fn candidate(identifier: &str, version: &str) -> PackageCandidate {
        PackageCandidate {
            identifier: identifier.to_string(),
            package_name: Some(identifier.to_string()),
            version: version.to_string(),
            description: None,
            publisher: None,
            manifest_path: None,
        }
    }

    #[test]
    fn resolved_install_prefers_requested_version() {
        let candidate = candidate("Microsoft.WindowsTerminal", "1.0.0");

        let resolved = resolved_install(&candidate, Some("2.0.0"));

        assert_eq!(resolved.identifier, candidate.identifier);
        assert_eq!(resolved.version, "2.0.0");
    }

    #[test]
    fn handle_selection_uses_preselected_candidate() {
        let mut ui = Ui::with_writer(Vec::new());
        let candidates = vec![
            candidate("Example.One", "1.0.0"),
            candidate("Example.Two", "2.0.0"),
        ];

        let resolved = handle_selection(
            &mut ui,
            &["example".to_string()],
            &candidates,
            None,
            Some(2),
        )
        .expect("selection should succeed");

        assert_eq!(resolved.identifier, "Example.Two");
        assert_eq!(resolved.version, "2.0.0");
    }

    #[test]
    fn handle_selection_rejects_out_of_range_preselection() {
        let mut ui = Ui::with_writer(Vec::new());
        let candidates = vec![candidate("Example.One", "1.0.0")];

        let error = handle_selection(
            &mut ui,
            &["example".to_string()],
            &candidates,
            None,
            Some(2),
        )
        .expect_err("selection should fail");

        assert!(
            error
                .to_string()
                .contains("install selection must be between 1 and 1")
        );
    }
}
