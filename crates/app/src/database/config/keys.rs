use std::env;

pub(crate) fn env_override(key: &str) -> Option<String> {
    env::var(env_override_name(key))
        .ok()
        .filter(|value| !value.trim().is_empty())
}

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
    use super::{env_override_name, section_key};

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
}
