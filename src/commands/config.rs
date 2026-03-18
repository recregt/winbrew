use anyhow::{Context, Result, anyhow, bail};

use crate::cli::ConfigCommand;
use crate::{database, ui::Ui};

pub fn run(command: ConfigCommand) -> Result<()> {
    let ui = Ui::new();
    ui.page_title("Config");

    match command {
        ConfigCommand::List => list(&ui),
        ConfigCommand::Get { key } => get(&ui, &key),
        ConfigCommand::Set { key, value } => set(&ui, &key, &value),
    }
}

fn list(ui: &Ui) -> Result<()> {
    let conn = database::lock_conn()?;
    let entries = database::config_list(&conn)?;

    if entries.is_empty() {
        ui.notice("No config values are set.");
        return Ok(());
    }

    for (key, value) in entries {
        ui.info(format!("{key} = {value}"));
    }

    Ok(())
}

fn get(ui: &Ui, key: &str) -> Result<()> {
    let conn = database::lock_conn()?;

    let value =
        database::config_get(&conn, key)?.ok_or_else(|| anyhow!("config key '{key}' not found"))?;

    ui.info(format!("{key} = {value}"));
    Ok(())
}

fn set(ui: &Ui, key: &str, value: &str) -> Result<()> {
    let clean_key = key.trim();
    if clean_key.is_empty() {
        bail!("config key cannot be empty");
    }

    let clean_value = value.trim();
    let conn = database::lock_conn()?;
    database::config_set(&conn, clean_key, clean_value).context("failed to update config")?;
    ui.success(format!("{clean_key} updated."));
    Ok(())
}
