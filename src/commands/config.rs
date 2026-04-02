use anyhow::{Context, Result};
use std::io::Write;

use crate::cli::ConfigCommand;
use crate::database::ConfigSource;
use crate::{services::config, ui::Ui};

pub fn run(command: ConfigCommand) -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("Configuration");

    match command {
        ConfigCommand::List => list(&mut ui),
        ConfigCommand::Get { key } => get(&mut ui, &key),
        ConfigCommand::Set { key, value } => set(&mut ui, &key, value.as_deref()),
    }
}

fn list<W: Write>(ui: &mut Ui<W>) -> Result<()> {
    let sections = config::list_sections().context("failed to load configuration")?;

    if sections.is_empty() {
        ui.notice("No configuration values are set.");
        return Ok(());
    }

    for section in sections {
        ui.notice(&section.title);
        ui.display_key_values(&section.entries);
    }

    Ok(())
}

fn get<W: Write>(ui: &mut Ui<W>, key: &str) -> Result<()> {
    let clean_key = key.trim();
    let (value, source) = config::get_value(clean_key)?;

    if source == ConfigSource::Env {
        ui.info(format!("{clean_key} = {value} (overridden by environment)"));
    } else {
        ui.info(format!("{clean_key} = {value}"));
    }
    Ok(())
}

fn set<W: Write>(ui: &mut Ui<W>, key: &str, value: Option<&str>) -> Result<()> {
    let clean_key = key.trim();

    let clean_value = match value {
        Some(value) => value.trim().to_string(),
        None => ui
            .prompt_text(&format!("Enter value for {clean_key}"), None)?
            .trim()
            .to_string(),
    };

    config::set_value(clean_key, &clean_value)?;
    ui.success(format!("{clean_key} updated."));
    Ok(())
}
