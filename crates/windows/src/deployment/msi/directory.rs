//! Resolves MSI `Directory` rows into absolute install paths.
//!
//! This module is intentionally narrow: it only handles the directory graph,
//! cycle detection, and the special handling for MSI's root-ish directory ids.
//! If the database omits a required row or forms a loop, the scan fails with a
//! contextual error instead of inventing a path.

use anyhow::{Context, Result, bail};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use super::{DirectoryRow, path::select_msi_name};

pub(super) fn resolve_directory_paths(
    rows: &HashMap<String, DirectoryRow>,
    install_root: &Path,
) -> Result<HashMap<String, PathBuf>> {
    // Resolve every directory id in the table into a concrete path.
    //
    // The walk is memoized so each directory is resolved once, and a
    // `visiting` set guards against recursive cycles in the MSI graph.
    let mut resolved = HashMap::new();
    let mut visiting = HashSet::new();

    for directory_id in rows.keys() {
        let _ = resolve_directory_path(
            directory_id,
            rows,
            &mut resolved,
            &mut visiting,
            install_root,
        )?;
    }

    Ok(resolved)
}

fn resolve_directory_path(
    directory_id: &str,
    rows: &HashMap<String, DirectoryRow>,
    resolved: &mut HashMap<String, PathBuf>,
    visiting: &mut HashSet<String>,
    install_root: &Path,
) -> Result<PathBuf> {
    // Resolve one directory id, following parents first and anchoring the
    // special MSI roots at the install root.
    if let Some(path) = resolved.get(directory_id) {
        return Ok(path.clone());
    }

    if !visiting.insert(directory_id.to_string()) {
        bail!("cycle detected in MSI Directory table at '{directory_id}'");
    }

    let row = rows
        .get(directory_id)
        .with_context(|| format!("missing MSI Directory row for '{directory_id}'"))?;

    let base = match row.parent.as_deref() {
        Some(parent) if !parent.is_empty() => {
            resolve_directory_path(parent, rows, resolved, visiting, install_root)?
        }
        _ if directory_id.eq_ignore_ascii_case("TARGETDIR")
            || directory_id.eq_ignore_ascii_case("SOURCEDIR") =>
        {
            install_root.to_path_buf()
        }
        _ => install_root.to_path_buf(),
    };

    let path = match select_msi_name(&row.default_dir) {
        Some(segment) => base.join(segment),
        None => base,
    };

    visiting.remove(directory_id);
    resolved.insert(directory_id.to_string(), path.clone());

    Ok(path)
}
