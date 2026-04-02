use anyhow::Result;

use crate::{services::remove, ui::Ui};

pub fn run(name: &str, yes: bool, force: bool) -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("Remove Package");

    ui.info(format!("Assessing impact for {name}..."));
    let plan = remove::plan_removal(name)?;

    if !plan.dependents.is_empty() {
        ui.warn(format!(
            "Caution: {} is required by: {}",
            plan.name,
            plan.dependents.join(", ")
        ));
    }

    if !should_proceed(&mut ui, &plan, yes, force)? {
        ui.notice("Removal aborted.");
        return Ok(());
    }

    ui.spinner(format!("Removing {}...", plan.name), || {
        remove::execute_removal(&plan, force)
    })?;

    ui.success(format!("Successfully removed {}.", plan.name));

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
        format!("Are you sure you want to remove {}?", plan.name)
    } else {
        format!(
            "Removal of {} may break other packages. Proceed anyway?",
            plan.name
        )
    };

    ui.confirm(&prompt, false)
}
