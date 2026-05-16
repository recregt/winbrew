use std::env;

pub(crate) fn env_override(key: &str) -> Option<String> {
    env_override_from(key, |name| env::var(name).ok())
}

fn env_override_from<F>(key: &str, lookup: F) -> Option<String>
where
    F: FnOnce(&str) -> Option<String>,
{
    lookup(&env_override_name(key)).filter(|value| !value.trim().is_empty())
}

/// Keep this mapping in sync with the known config sections owned by the config registry.
pub(crate) fn section_key(section_title: &str, key: &str) -> String {
    let section = section_title.to_lowercase();

    match section.as_str() {
        "core" | "paths" => format!("{section}.{key}"),
        _ => key.to_string(),
    }
}

fn env_override_name(key: &str) -> String {
    format!("WINBREW_{}", key.replace('.', "_").to_uppercase())
}

#[cfg(test)]
mod tests {
    use super::{env_override_from, env_override_name, section_key};

    #[test]
    fn section_key_uses_section_prefix_for_known_sections() {
        assert_eq!(section_key("Core", "log_level"), "core.log_level");
        assert_eq!(section_key("Paths", "root"), "paths.root");
    }

    #[test]
    fn section_key_leaves_unknown_sections_unmodified() {
        assert_eq!(section_key("Custom", "value"), "value");
    }

    #[test]
    fn env_override_name_returns_canonical_name() {
        assert_eq!(
            env_override_name("paths.packages"),
            "WINBREW_PATHS_PACKAGES"
        );
    }

    #[test]
    fn env_override_ignores_blank_values() {
        assert_eq!(
            env_override_from("test.key", |_| Some("   ".to_string())),
            None
        );
    }

    #[test]
    fn env_override_returns_non_blank_values() {
        assert_eq!(
            env_override_from("test.key", |_| Some("  value  ".to_string())),
            Some("  value  ".to_string())
        );
    }

    #[test]
    fn env_override_returns_none_when_missing() {
        assert_eq!(env_override_from("test.key", |_| None), None);
    }
}
