use anyhow::Result;

use crate::{operations::remover, ui::Ui};

pub fn run(name: &str, yes: bool) -> Result<()> {
    let ui = Ui::new();
    ui.page_title("Remove");

    let dependents = remover::find_dependents(name)?;
    if !dependents.is_empty() {
        ui.notice(format!(
            "Warning: {name} is required by installed package(s): {}",
            dependents.join(", ")
        ));

        if yes {
            ui.notice("Proceeding because --yes was provided.");
        }
    }

    if !yes {
        let prompt = if dependents.is_empty() {
            "Do you want to remove this package and its shims?"
        } else {
            "This removal can break dependent packages. Remove anyway?"
        };

        let confirmed = ui.confirm(prompt, false)?;

        if !confirmed {
            ui.notice("Aborted.");
            return Ok(());
        }
    }

    ui.spinner(format!("Removing {name}..."), || remover::remove(name))?;

    ui.success(format!("{name} was removed."));

    Ok(())
}
