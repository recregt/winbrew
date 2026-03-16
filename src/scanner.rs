use anyhow::Result;
use rayon::prelude::*;
use regex::RegexBuilder;
use std::{collections::HashSet, env, path::PathBuf};

use crate::uninstall::{Hive, uninstall_roots};

pub struct ScanResult {
    pub registry_matches: Vec<RegistryMatch>,
    pub directory_matches: Vec<PathBuf>,
}

pub struct RegistryMatch {
    pub hive: Hive,
    pub full_key_path: String,
    pub root_label: &'static str,
    pub key_name: String,
    pub display_name: String,
}

pub fn collect(name: &str) -> Result<ScanResult> {
    let pattern = regex::escape(name);
    let re = RegexBuilder::new(&pattern).case_insensitive(true).build()?;

    let (registry_matches, mut directory_matches) = rayon::join(
        || fetch_registry_matches(&re),
        || fetch_directory_matches(&re),
    );

    directory_matches.sort();

    Ok(ScanResult {
        registry_matches,
        directory_matches,
    })
}

fn fetch_registry_matches(re: &regex::Regex) -> Vec<RegistryMatch> {
    let mut matches = Vec::new();

    for root in uninstall_roots() {
        for key_result in root.key.enum_keys() {
            let Ok(key_name) = key_result else { continue };
            let Ok(app_key) = root.key.open_subkey(&key_name) else {
                continue;
            };
            let Ok(display_name) = app_key.get_value::<String, _>("DisplayName") else {
                continue;
            };

            if re.is_match(&display_name) {
                let full_key_path = format!("{}\\{}", root.key_path, key_name);
                matches.push(RegistryMatch {
                    hive: root.hive,
                    full_key_path,
                    root_label: root.label,
                    key_name,
                    display_name,
                });
            }
        }
    }

    matches
}

fn fetch_directory_matches(re: &regex::Regex) -> Vec<PathBuf> {
    candidate_dirs()
        .into_par_iter()
        .flat_map_iter(|dir| {
            std::fs::read_dir(dir)
                .into_iter()
                .flatten()
                .filter_map(Result::ok)
                .filter(|entry| {
                    let is_dir = entry.file_type().map(|ft| ft.is_dir()).unwrap_or(false);
                    if !is_dir {
                        return false;
                    }
                    re.is_match(&entry.file_name().to_string_lossy())
                })
                .map(|entry| entry.path())
        })
        .collect()
}

fn candidate_dirs() -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    let mut dirs = Vec::new();

    for var in [
        "PROGRAMFILES",
        "PROGRAMW6432",
        "PROGRAMFILES(X86)",
        "APPDATA",
        "LOCALAPPDATA",
    ] {
        if let Ok(val) = env::var(var) {
            let path = PathBuf::from(val);
            if seen.insert(path.clone()) {
                dirs.push(path);
            }
        }
    }

    if let Ok(local) = env::var("LOCALAPPDATA")
        && let Some(parent) = PathBuf::from(local).parent()
    {
        let locallow = parent.join("LocalLow");
        if seen.insert(locallow.clone()) {
            dirs.push(locallow);
        }
    }

    dirs
}
