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

    ui.spinner(format!("Removing {}...", plan.package.name), || {
        remove::execute_removal(&plan, force)
    })?;

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
