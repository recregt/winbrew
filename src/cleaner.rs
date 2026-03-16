use crate::scanner::{RegistryMatch, ScanResult};
use anyhow::Result;
use rayon::prelude::*;

pub struct CleanReport {
    pub success_count: usize,
    pub failures: Vec<String>,
}

pub fn execute_clean(scan_result: &ScanResult) -> CleanReport {
    let reg_results: Vec<_> = scan_result
        .registry_matches
        .par_iter()
        .map(|reg| {
            delete_registry_match(reg).map_err(|e| {
                format!(
                    "Registry [{}\\{} | hive={} | path={}]: {}",
                    reg.root_label, reg.key_name, reg.hive, reg.full_key_path, e
                )
            })
        })
        .collect();

    let dir_results: Vec<_> = scan_result
        .directory_matches
        .par_iter()
        .map(|dir| {
            std::fs::remove_dir_all(dir)
                .map_err(|e| format!("Directory [{}]: {}", dir.display(), e))
        })
        .collect();

    let mut success_count = 0;
    let mut failures = Vec::new();

    for result in reg_results.into_iter().chain(dir_results) {
        match result {
            Ok(()) => success_count += 1,
            Err(e) => failures.push(e),
        }
    }

    CleanReport {
        success_count,
        failures,
    }
}

fn delete_registry_match(reg: &RegistryMatch) -> Result<()> {
    let root = reg.hive.open();
    root.delete_subkey_all(&reg.full_key_path)?;
    Ok(())
}
