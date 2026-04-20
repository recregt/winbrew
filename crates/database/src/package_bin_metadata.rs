use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, params};
use std::collections::HashSet;

pub fn get_package_bin_metadata(conn: &Connection, package_name: &str) -> Result<Option<String>> {
    let mut stmt = conn.prepare(
        "SELECT bin_json
         FROM package_bin_lists
         WHERE package_name = ?1",
    )?;

    stmt.query_row(params![package_name], |row| row.get::<_, String>(0))
        .optional()
        .context("failed to read package bin metadata")
}

pub fn sync_package_bin_metadata(
    conn: &Connection,
    package_name: &str,
    raw_bin_metadata: Option<&str>,
) -> Result<()> {
    let bin_paths = parse_bin_paths(raw_bin_metadata)?;
    let bin_json =
        serde_json::to_string(&bin_paths).context("failed to serialize package bin metadata")?;

    conn.execute(
        "INSERT INTO package_bin_lists (package_name, bin_json)
         VALUES (?1, ?2)
         ON CONFLICT(package_name) DO UPDATE SET
             bin_json = excluded.bin_json",
        params![package_name, bin_json],
    )
    .context("failed to upsert package bin metadata")?;

    Ok(())
}

fn parse_bin_paths(raw_bin_metadata: Option<&str>) -> Result<Vec<String>> {
    let Some(raw_bin_metadata) = raw_bin_metadata else {
        return Ok(Vec::new());
    };

    let bin_paths: Vec<String> = serde_json::from_str(raw_bin_metadata)
        .with_context(|| "failed to parse package bin metadata JSON")?;

    Ok(normalize_bin_paths(bin_paths))
}

fn normalize_bin_paths<I, S>(bin_paths: I) -> Vec<String>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    let mut seen = HashSet::new();
    let mut normalized = Vec::new();

    for bin_path in bin_paths {
        let normalized_bin_path = normalize_path_separators(bin_path.as_ref().trim());
        if normalized_bin_path.is_empty() {
            continue;
        }

        let dedupe_key = normalized_bin_path.to_ascii_lowercase();
        if seen.insert(dedupe_key) {
            normalized.push(normalized_bin_path);
        }
    }

    normalized
}

fn normalize_path_separators(path: &str) -> String {
    path.replace('/', "\\")
}
