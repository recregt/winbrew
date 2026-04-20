use anyhow::{Context, Result};
use rusqlite::{Connection, Error as SqlError, ErrorCode, OptionalExtension, params};
use std::collections::BTreeMap;
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error(
    "command '{command_name}' was claimed by another install while this install was in progress"
)]
pub struct CommandRegistryConflictError {
    pub command_name: String,
}

pub fn parse_command_names(raw_commands: Option<&str>) -> Result<Vec<String>> {
    let Some(raw_commands) = raw_commands else {
        return Ok(Vec::new());
    };

    let commands: Vec<String> = serde_json::from_str(raw_commands)
        .with_context(|| "failed to parse exposed commands JSON")?;

    Ok(normalize_command_names(commands))
}

pub fn find_command_owner(conn: &Connection, command_name: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT package_name
         FROM command_registry
         WHERE command_name = ?1",
    )?;

    stmt.query_row(params![command_name], |row| row.get::<_, String>(0))
        .optional()
        .context("failed to read command registry")
}

pub fn get_package_command_names(
    conn: &Connection,
    package_name: &str,
) -> Result<Option<Vec<String>>> {
    let mut stmt = conn.prepare(
        "SELECT commands_json
         FROM package_command_lists
         WHERE package_name = ?1",
    )?;

    let commands_json = stmt
        .query_row(params![package_name], |row| row.get::<_, String>(0))
        .optional()
        .context("failed to read package command list")?;

    let Some(commands_json) = commands_json else {
        return Ok(None);
    };

    Ok(Some(parse_command_names(Some(commands_json.as_str()))?))
}

pub fn list_commands_for_package(conn: &Connection, package_name: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT command_name
         FROM command_registry
         WHERE package_name = ?1
         ORDER BY command_name ASC",
    )?;

    stmt.query_map(params![package_name], |row| row.get::<_, String>(0))?
        .collect::<std::result::Result<Vec<_>, _>>()
        .context("failed to read package commands")
}

pub fn sync_package_commands(
    conn: &Connection,
    package_name: &str,
    raw_commands: Option<&str>,
) -> Result<()> {
    let commands = parse_command_names(raw_commands)?;
    let commands_json =
        serde_json::to_string(&commands).context("failed to serialize package command list")?;

    conn.execute(
        "INSERT INTO package_command_lists (package_name, commands_json)
         VALUES (?1, ?2)
         ON CONFLICT(package_name) DO UPDATE SET
             commands_json = excluded.commands_json",
        params![package_name, commands_json],
    )
    .context("failed to upsert package command list")?;

    conn.execute(
        "DELETE FROM command_registry WHERE package_name = ?1",
        params![package_name],
    )
    .context("failed to clear command registry rows")?;

    let mut stmt = conn.prepare(
        "INSERT INTO command_registry (command_name, package_name)
         VALUES (?1, ?2)",
    )?;

    for command_name in commands {
        match stmt.execute(params![command_name.as_str(), package_name]) {
            Ok(_) => {}
            Err(err) if is_unique_conflict(&err) => {
                return Err(CommandRegistryConflictError { command_name }.into());
            }
            Err(err) => return Err(err).context("failed to update command registry"),
        }
    }

    Ok(())
}

fn normalize_command_names<I, S>(commands: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut normalized = BTreeMap::new();

    for command in commands {
        let trimmed = command.as_ref().trim();
        if trimmed.is_empty() {
            continue;
        }

        normalized
            .entry(trimmed.to_ascii_lowercase())
            .or_insert_with(|| trimmed.to_string());
    }

    normalized.into_values().collect()
}

fn is_unique_conflict(err: &SqlError) -> bool {
    matches!(err, SqlError::SqliteFailure(error, _) if error.code == ErrorCode::ConstraintViolation)
}
