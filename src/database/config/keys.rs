use std::env;

pub(crate) fn env_override(key: &str) -> Option<String> {
    env_override_names(key)
        .into_iter()
        .find_map(|name| env::var(&name).ok())
        .filter(|value| !value.trim().is_empty())
}

pub(crate) fn section_key(section_title: &str, key: &str) -> String {
    match section_title.to_lowercase().as_str() {
        "core" => format!("core.{key}"),
        "paths" => format!("paths.{key}"),
        "sources" => format!("sources.{key}"),
        _ => key.to_string(),
    }
}

fn env_override_names(key: &str) -> Vec<String> {
    let mut names = vec![format!("WINBREW_{}", key.replace('.', "_").to_uppercase())];

    match key {
        "core.log_level" => names.push("WINBREW_LOG_LEVEL".to_string()),
        "core.file_log_level" => names.push("WINBREW_FILE_LOG_LEVEL".to_string()),
        "core.auto_update" => names.push("WINBREW_AUTO_UPDATE".to_string()),
        "core.confirm_remove" => names.push("WINBREW_CONFIRM_REMOVE".to_string()),
        "core.default_yes" => names.push("WINBREW_DEFAULT_YES".to_string()),
        "core.color" => names.push("WINBREW_COLOR".to_string()),
        "core.download_timeout" => names.push("WINBREW_DOWNLOAD_TIMEOUT".to_string()),
        "core.concurrent_downloads" => {
            names.push("WINBREW_THREADS".to_string());
            names.push("WINBREW_CONCURRENT_DOWNLOADS".to_string());
        }
        "core.github_token" => names.push("WINBREW_GITHUB_TOKEN".to_string()),
        "core.proxy" => names.push("WINBREW_PROXY".to_string()),
        "paths.root" => names.push("WINBREW_ROOT".to_string()),
        "sources.primary" => names.push("WINBREW_PRIMARY_SOURCE".to_string()),
        "sources.winget.url" => names.push("WINBREW_REGISTRY_URL".to_string()),
        "sources.winget.format" => names.push("WINBREW_REGISTRY_FORMAT".to_string()),
        "sources.winget.manifest_kind" => names.push("WINBREW_MANIFEST_KIND".to_string()),
        "sources.winget.manifest_path_template" => {
            names.push("WINBREW_MANIFEST_PATH_TEMPLATE".to_string())
        }
        "sources.winget.enabled" => names.push("WINBREW_WINGET_ENABLED".to_string()),
        _ => {}
    }

    names
}
