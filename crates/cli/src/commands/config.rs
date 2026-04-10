//! Configuration command handlers.
//!
//! This module routes `config` subcommands, reads effective values,
//! updates persisted values, and removes overrides when requested.

use anyhow::{Context, Result};
use std::io::Write;

use crate::cli::ConfigCommand;
use crate::commands::error::reported;
use crate::database::Config;
use crate::{AppContext, Ui, app::config};

/// Dispatches a `config` subcommand to the appropriate handler.
pub fn run(ctx: &AppContext, config: &mut Config, command: ConfigCommand) -> Result<()> {
    let mut ui = Ui::new(ctx.ui);
    ui.page_title("Configuration");

    match command {
        ConfigCommand::List => list(config, &mut ui),
        ConfigCommand::Get { key } => get(config, &mut ui, key.trim()),
        ConfigCommand::Set { key, value } => set(config, &mut ui, key.trim(), value.as_deref()),
        ConfigCommand::Unset { key } => unset(config, &mut ui, key.trim()),
    }
}

/// Lists all configuration sections and their values.
fn list<W: Write>(config: &Config, ui: &mut Ui<W>) -> Result<()> {
    let sections = config::list_sections(config)?;

    if sections.is_empty() {
        ui.notice("No configuration values are set.");
        return Ok(());
    }

    for config::ConfigSection { title, entries } in &sections {
        ui.notice(title);
        ui.display_key_values(entries);
    }

    Ok(())
}

/// Displays the effective value for a configuration key.
fn get<W: Write>(config: &Config, ui: &mut Ui<W>, key: &str) -> Result<()> {
    let value = config::get_display_value(config, key)
        .with_context(|| format!("Failed to retrieve configuration for key: '{key}'"))?;

    ui.info(format_args!(
        "{key} = {}{}",
        value.value,
        source_suffix(value.source)
    ));
    Ok(())
}

/// Stores a configuration value, either from the CLI argument or an interactive prompt.
///
/// Empty values are rejected so callers use `unset` to remove keys instead of
/// silently writing blank entries.
fn set<W: Write>(
    config: &mut Config,
    ui: &mut Ui<W>,
    key: &str,
    value: Option<&str>,
) -> Result<()> {
    let owned_prompt;
    let clean_value = match value {
        Some(value) => value.trim(),
        None => {
            owned_prompt = ui.prompt_text(&format!("Enter value for {key}"), None)?;
            owned_prompt.trim()
        }
    };

    if clean_value.is_empty() {
        return Err(reported(
            "Empty value is not allowed. Use 'unset' to remove a configuration key.",
        ));
    }

    config::set_value(config, key, clean_value)
        .with_context(|| format!("Failed to set configuration for key: '{key}'"))?;

    ui.success(format!("{key} updated."));
    Ok(())
}

/// Removes a configuration key by restoring its default value.
fn unset<W: Write>(config: &mut Config, ui: &mut Ui<W>, key: &str) -> Result<()> {
    config::unset_value(config, key)
        .with_context(|| format!("Failed to remove configuration for key: '{key}'"))?;

    ui.success(format!("{key} removed."));
    Ok(())
}

/// Returns the visual suffix used when a value is overridden by the environment.
fn source_suffix(source: config::ConfigValueSource) -> &'static str {
    match source {
        config::ConfigValueSource::Env => " (overridden by environment)",
        config::ConfigValueSource::File => "",
    }
}
