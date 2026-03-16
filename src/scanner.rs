use anyhow::Result;
use std::{collections::HashSet, env, path::PathBuf};

use crate::uninstall::{uninstall_roots, Hive, UninstallRoot};

pub struct ScanResult {
    pub registry_matches: Vec<RegistryMatch>,
    pub directory_matches: Vec<PathBuf>,
}

pub struct RegistryMatch {
    pub hive: Hive,
    pub uninstall_key_path: &'static str,
    pub root_label: &'static str,
    pub key_name: String,
    pub display_name: String,
}

pub fn scan(name: &str) -> Result<()> {
    let result = collect(name);
    display_scan_result(&result);

    Ok(())
}

pub fn collect(name: &str) -> ScanResult {
    let query = name.to_lowercase();

    ScanResult {
        registry_matches: registry_matches(&query),
        directory_matches: matching_directories(&query),
    }
}

pub fn display_scan_result(result: &ScanResult) {
    println!("[Registry]");

    if result.registry_matches.is_empty() {
        println!("  (nothing found)");
    } else {
        for registry_match in &result.registry_matches {
            println!(
                "  [{}\\{}]  {}",
                registry_match.root_label, registry_match.key_name, registry_match.display_name
            );
        }
    }

    println!("\n[Directories]");

    if result.directory_matches.is_empty() {
        println!("  (nothing found)");
    } else {
        for directory in &result.directory_matches {
            println!("  {}", directory.display());
        }
    }
}

fn registry_matches(query: &str) -> Vec<RegistryMatch> {
    uninstall_roots()
        .into_iter()
        .flat_map(|root| scan_reg_key(&root, query))
        .collect()
}

fn scan_reg_key(root: &UninstallRoot, query: &str) -> Vec<RegistryMatch> {
    root.key
        .enum_keys()
        .flatten()
        .filter_map(|key_name| {
            let app_key = root.key.open_subkey(&key_name).ok()?;
            let display_name: String = app_key.get_value("DisplayName").ok()?;

            display_name.to_lowercase().contains(query).then_some(RegistryMatch {
                hive: root.hive,
                uninstall_key_path: root.key_path,
                root_label: root.label,
                key_name,
                display_name,
            })
        })
        .collect()
}

fn matching_directories(query: &str) -> Vec<PathBuf> {
    candidate_dirs()
        .into_iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .flatten()
        .filter(|entry| {
            let name = entry.file_name();
            name.to_string_lossy().to_lowercase().contains(query) && entry.path().is_dir()
        })
        .map(|entry| entry.path())
        .collect()
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut dirs = Vec::new();

    for var in ["PROGRAMFILES", "PROGRAMW6432", "PROGRAMFILES(X86)", "APPDATA", "LOCALAPPDATA"] {
        if let Ok(val) = env::var(var) {
            let path = PathBuf::from(val);
            if seen.insert(path.clone()) {
                dirs.push(path);
            }
        }
    }

    if let Ok(local) = env::var("LOCALAPPDATA") {
        let local_path = PathBuf::from(local);
        if let Some(appdata_root) = local_path.parent() {
            let locallow = appdata_root.join("LocalLow");
            if seen.insert(locallow.clone()) {
                dirs.push(locallow);
            }
        }
    }

    dirs
}
