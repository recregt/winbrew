use anyhow::{anyhow, Result};
use dialoguer::Confirm;

use crate::scanner::{self, RegistryMatch, ScanResult};

pub fn clean(name: &str, dry_run: bool, yes: bool) -> Result<()> {
    let result = scanner::collect(name);
    scanner::display_scan_result(&result);

    if is_empty(&result) {
        println!("\nNothing to clean.");
        return Ok(());
    }

    if dry_run {
        println!("\nDry run: no changes were made.");
        return Ok(());
    }

    if !yes
        && !Confirm::new()
            .with_prompt("Delete these items?")
            .default(false)
            .interact()?
    {
        println!("\nAborted.");
        return Ok(());
    }

    let mut deleted_count = 0usize;
    let mut failures = Vec::new();

    for registry_match in &result.registry_matches {
        match delete_registry_match(registry_match) {
            Ok(()) => {
                println!("deleted registry: [{}\\{}]", registry_match.root_label, registry_match.key_name);
                deleted_count += 1;
            }
            Err(error) => failures.push(format!(
                "failed registry [{}\\{}]: {error}",
                registry_match.root_label, registry_match.key_name
            )),
        }
    }

    for directory in &result.directory_matches {
        match std::fs::remove_dir_all(directory) {
            Ok(()) => {
                println!("deleted directory: {}", directory.display());
                deleted_count += 1;
            }
            Err(error) => failures.push(format!(
                "failed directory {}: {error}",
                directory.display()
            )),
        }
    }

    println!("\nDeleted {deleted_count} item(s).");

    if failures.is_empty() {
        return Ok(());
    }

    println!("\nFailures:");
    for failure in &failures {
        println!("  {failure}");
    }

    Err(anyhow!("Failed to remove {} item(s).", failures.len()))
}

fn is_empty(result: &ScanResult) -> bool {
    result.registry_matches.is_empty() && result.directory_matches.is_empty()
}

fn delete_registry_match(registry_match: &RegistryMatch) -> Result<()> {
    let root = registry_match.hive.open();
    let key_path = format!("{}\\{}", registry_match.uninstall_key_path, registry_match.key_name);

    root.delete_subkey_all(&key_path)?;
    Ok(())
}
