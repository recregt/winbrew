use anyhow::Result;

use crate::{AppContext, services::remove, ui::Ui};

pub fn run(ctx: &AppContext, name: &str, yes: bool, force: bool) -> Result<()> {
    let mut ui = Ui::new(ctx.ui);
    ui.page_title("Remove Package");

    ui.info(format!("Assessing impact for {name}..."));
    let plan = remove::plan_removal(name)?;

    if !plan.dependents.is_empty() {
        ui.warn(format!(
            "Caution: {} is required by: {}",
            plan.package.name,
            plan.dependents.join(", ")
        ));
    }

    if !should_proceed(&mut ui, &plan, yes, force)? {
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
                return Err(anyhow::Error::msg(format!(
                    "cannot remove '{name}' because it is required by: {}",
                    dependents
                )));
            }
            remove::RemovalError::UnsupportedPackageType { kind } => {
                ui.error(format!("unsupported package type: {kind}"));
                return Err(anyhow::Error::msg(format!(
                    "unsupported package type: {kind}"
                )));
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
) -> Result<bool> {
    if force || yes {
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

    ui.confirm(&prompt, false)
}
