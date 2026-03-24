use std::env;

use super::registry;

pub(crate) fn env_override(key: &str) -> Option<String> {
    // Prefer canonical names first so explicit config-specific overrides win over legacy aliases.
    env_override_names(key)
        .into_iter()
        .find_map(|name| env::var(&name).ok())
        .filter(|value| !value.trim().is_empty())
}

pub(crate) fn section_key(section_title: &str, key: &str) -> String {
    let section = section_title.to_lowercase();

    match section.as_str() {
        "core" | "paths" | "sources" => format!("{section}.{key}"),
        _ => key.to_string(),
    }
}

fn env_override_names(key: &str) -> Vec<String> {
    let canonical = format!("WINBREW_{}", key.replace('.', "_").to_uppercase());

    let aliases = registry::find(key)
        .map(|def| def.env_aliases)
        .unwrap_or(&[]);

    std::iter::once(canonical)
        .chain(aliases.iter().copied().map(str::to_string))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{env_override_names, section_key};

    #[test]
    fn section_key_uses_section_prefix_for_known_sections() {
        assert_eq!(section_key("Core", "log_level"), "core.log_level");
        assert_eq!(section_key("Paths", "root"), "paths.root");
        assert_eq!(section_key("Sources", "primary"), "sources.primary");
    }

    #[test]
    fn section_key_leaves_unknown_sections_unmodified() {
        assert_eq!(section_key("Custom", "value"), "value");
    }

    #[test]
    fn env_override_names_prefers_canonical_before_aliases() {
        assert_eq!(
            env_override_names("core.concurrent_downloads"),
            vec![
                "WINBREW_CORE_CONCURRENT_DOWNLOADS".to_string(),
                "WINBREW_THREADS".to_string(),
                "WINBREW_CONCURRENT_DOWNLOADS".to_string(),
            ]
        );
    }
}
