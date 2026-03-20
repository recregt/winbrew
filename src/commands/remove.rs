use anyhow::Result;

use crate::{database, services::remover, ui::Ui};

pub fn run(name: &str, yes: bool, force: bool) -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("Remove");

    let conn = database::lock_conn()?;
    let dependents = remover::find_dependents(name, &conn)?;
    if !dependents.is_empty() {
        ui.notice(format!(
            "Warning: {name} is required by installed package(s): {}",
            dependents.join(", ")
        ));

        if force {
            ui.notice("Proceeding because --force was provided.");
        } else if yes {
            ui.notice("Proceeding because --yes was provided.");
        }
    }

    if !yes && !force {
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

    drop(conn);

    ui.spinner(format!("Removing {name}..."), || {
        remover::remove(name, force)
    })?;

    ui.success(format!("{name} was removed."));

    Ok(())
}
