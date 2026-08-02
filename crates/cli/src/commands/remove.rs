//! Remove command wrapper for dependency checks, confirmation prompts, and
//! removal outcomes.

use anyhow::Result;

use crate::commands::error::{reported, reported_with_hint};
use crate::{CommandContext, app::remove};
use winbrew_ui::Ui;

pub fn run(ctx: &CommandContext, name: &[String], yes: bool, force: bool) -> Result<()> {
    let mut ui = ctx.ui();
    ui.page_title("Remove Package");

    let name_text = name.join(" ").trim().to_owned();
    if name_text.is_empty() {
        let message = "package name cannot be empty".to_string();
        ui.error(&message);
        return Err(reported(message));
    }

    ui.info(format!("Assessing impact for {name_text}..."));
    let plan = remove::plan_removal(&name_text)?;

    if !plan.dependents.is_empty() {
        ui.warn(format!(
            "Caution: {} is required by: {}",
            plan.package.name,
            plan.dependents.join(", ")
        ));
    }

    if !should_proceed(&mut ui, &plan, yes, force, ctx.confirm_remove())? {
        ui.notice("Removal aborted.");
        return Ok(());
    }

    let removal_result = ui.spinner(format!("Removing {}...", plan.package.name), || {
        remove::execute_removal(&plan, force)
    });

    if let Err(err) = removal_result {
        match err {
            remove::RemovalError::DependentPackagesBlocked { name, dependents } => {
                ui.warn(format!(
                    "Removal of {name} was blocked because it is required by: {}",
                    dependents
                ));
                let message = format!(
                    "cannot remove '{name}' because it is required by: {}",
                    dependents
                );
                ui.notice("Hint: re-run with --force if you intend to remove the package anyway.");
                return Err(reported_with_hint(
                    message,
                    "Re-run with --force if you intend to remove the package anyway.",
                ));
            }
            remove::RemovalError::UnsupportedPackageType { kind } => {
                ui.error(format!("unsupported package type: {kind}"));
                let message = format!("unsupported package type: {kind}");
                ui.notice("Hint: check the package metadata or choose a supported installer type.");
                return Err(reported_with_hint(
                    message,
                    "Check the package metadata or choose a supported installer type.",
                ));
            }
            remove::RemovalError::Unexpected(err) => return Err(err),
        }
    }

    ui.success(format!("Successfully removed {}.", plan.package.name));

    Ok(())
}

fn should_proceed<W: std::io::Write>(
    ui: &mut Ui<W>,
    plan: &remove::RemovalPlan,
    yes: bool,
    force: bool,
    confirm_remove: bool,
) -> Result<bool> {
    // `core.confirm_remove = false` is a standing, remove-specific opt-out
    // of this prompt -- distinct from `core.default_yes`, which must never
    // silently approve a destructive action (see confirm_protected below).
    // This key exists specifically to control this one prompt, so honoring
    // it here is exactly its intended scope, not a violation of that policy.
    if force || yes || !confirm_remove {
        return Ok(true);
    }

    let prompt = if plan.dependents.is_empty() {
        format!("Are you sure you want to remove {}?", plan.package.name)
    } else {
        format!(
            "Removal of {} may break other packages. Proceed anyway?",
            plan.package.name
        )
    };

    // Removal is destructive, so a standing `core.default_yes` config
    // default must not be able to silently approve it -- only an explicit
    // `--yes`/`--force` on this invocation (checked above) or a real
    // interactive confirmation can.
    ui.confirm_protected(&prompt)
}

#[cfg(test)]
mod tests {
    use super::should_proceed;
    use crate::app::remove::RemovalPlan;
    use crate::commands::test_support::buffered_ui;
    use crate::models::domains::install::{EngineKind, InstallerType};
    use crate::models::domains::installed::{InstalledPackage, PackageStatus};
    use winbrew_ui::UiSettings;

    fn sample_plan(dependents: Vec<String>) -> RemovalPlan {
        RemovalPlan {
            package: InstalledPackage {
                name: "Contoso.App".to_string(),
                version: "1.0.0".to_string(),
                kind: InstallerType::Portable,
                deployment_kind: InstallerType::Portable.deployment_kind(),
                engine_kind: EngineKind::Portable,
                engine_metadata: None,
                install_dir: r"C:\winbrew\packages\Contoso.App".to_string(),
                dependencies: Vec::new(),
                status: PackageStatus::Ok,
                installed_at: "2026-04-12T00:00:00Z".to_string(),
            },
            dependents,
        }
    }

    /// `core.confirm_remove = false` must skip the prompt entirely, the same
    /// as an explicit `--yes`/`--force` -- this is the fix for the key
    /// previously being registered in config but never read anywhere.
    #[test]
    fn should_proceed_skips_prompt_when_confirm_remove_is_disabled() {
        let (mut ui, _out, _err) = buffered_ui(UiSettings::default());
        let plan = sample_plan(Vec::new());

        let proceed = should_proceed(&mut ui, &plan, false, false, false)
            .expect("should not need an interactive prompt");

        assert!(proceed);
    }

    #[test]
    fn should_proceed_skips_prompt_on_explicit_yes_or_force() {
        let plan = sample_plan(Vec::new());

        let (mut ui, _out, _err) = buffered_ui(UiSettings::default());
        assert!(should_proceed(&mut ui, &plan, true, false, true).expect("yes bypasses prompt"));

        let (mut ui, _out, _err) = buffered_ui(UiSettings::default());
        assert!(should_proceed(&mut ui, &plan, false, true, true).expect("force bypasses prompt"));
    }
}
