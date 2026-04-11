use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub(super) fn normalize_path(path: &Path) -> String {
    let raw = path.to_string_lossy();
    let stripped = raw
        .strip_prefix(r"\\?\UNC\")
        .map(|value| format!(r"\\{}", value))
        .or_else(|| raw.strip_prefix(r"\\?\").map(ToOwned::to_owned))
        .unwrap_or_else(|| raw.to_string());

    stripped.replace('\\', "/").to_ascii_lowercase()
}

pub(super) fn normalize_registry_key_path(path: &str) -> String {
    path.trim().to_ascii_lowercase()
}

pub(super) fn select_msi_name(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() || value == "." {
        return None;
    }

    let selected = match value.split_once('|') {
        Some((short_name, long_name)) => {
            let long_name = long_name.trim();
            let short_name = short_name.trim();

            if !long_name.is_empty() && long_name != "." {
                long_name
            } else if !short_name.is_empty() && short_name != "." {
                short_name
            } else {
                return None;
            }
        }
        None => value,
    };

    if selected.is_empty() || selected == "." {
        None
    } else {
        Some(selected.to_string())
    }
}

pub(super) fn resolve_reference_path(
    reference: &str,
    directory_paths: &HashMap<String, PathBuf>,
    file_paths: &HashMap<String, PathBuf>,
) -> Option<PathBuf> {
    let reference = reference.trim();
    if reference.is_empty() {
        return None;
    }

    if let Some(key) = reference
        .strip_prefix("[#")
        .and_then(|value| value.strip_suffix(']'))
    {
        return file_paths
            .get(key)
            .cloned()
            .or_else(|| directory_paths.get(key).cloned());
    }

    if let Some(rest) = reference.strip_prefix('[')
        && let Some((key, suffix)) = rest.split_once(']')
    {
        let base = file_paths
            .get(key)
            .cloned()
            .or_else(|| directory_paths.get(key).cloned())?;
        let suffix = suffix.trim_start_matches(['\\', '/']);

        return Some(if suffix.is_empty() {
            base
        } else {
            base.join(suffix)
        });
    }

    if let Some(path) = file_paths.get(reference) {
        return Some(path.clone());
    }

    if let Some(path) = directory_paths.get(reference) {
        return Some(path.clone());
    }

    if reference.contains('\\') || reference.contains('/') || reference.contains(':') {
        return Some(PathBuf::from(reference));
    }

    None
}

#[cfg(test)]
mod tests {
    use super::{normalize_path, normalize_registry_key_path, select_msi_name};
    use std::path::Path;

    #[test]
    fn select_msi_name_prefers_long_name() {
        assert_eq!(
            select_msi_name("SHORT|Long Name"),
            Some("Long Name".to_string())
        );
    }

    #[test]
    fn select_msi_name_handles_plain_values() {
        assert_eq!(
            select_msi_name("FolderName"),
            Some("FolderName".to_string())
        );
        assert_eq!(select_msi_name("SHORTNAM|."), Some("SHORTNAM".to_string()));
        assert_eq!(select_msi_name("."), None);
        assert_eq!(select_msi_name(""), None);
    }

    #[test]
    fn normalize_path_lowercases_and_uses_forward_slashes() {
        assert_eq!(
            normalize_path(Path::new(r"C:\Tools\Demo\bin\App.EXE")),
            "c:/tools/demo/bin/app.exe"
        );
    }

    #[test]
    fn normalize_registry_key_path_lowercases() {
        assert_eq!(
            normalize_registry_key_path(r"Software\Demo\Config"),
            "software\\demo\\config"
        );
    }
}
