use anyhow::Result;
use regex::RegexBuilder;

use crate::windows::uninstall::uninstall_roots;

/// Holds complete app info for display and filtering.
#[derive(Debug, Eq, PartialEq)]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    pub publisher: String,
}

pub fn collect_installed_apps(filter: Option<&str>) -> Result<Vec<AppInfo>> {
    let pattern = filter
        .map(|f| {
            RegexBuilder::new(&regex::escape(f))
                .case_insensitive(true)
                .build()
        })
        .transpose()?;

    let mut apps = Vec::new();

    for root in uninstall_roots() {
        for key_result in root.key.enum_keys() {
            let Ok(key_name) = key_result else { continue };
            let Ok(app_key) = root.key.open_subkey(&key_name) else {
                continue;
            };

            let Ok(name) = app_key.get_value::<String, _>("DisplayName") else {
                continue;
            };

            if pattern.as_ref().is_some_and(|re| !re.is_match(&name)) {
                continue;
            }

            let version = app_key
                .get_value::<String, _>("DisplayVersion")
                .unwrap_or_default();
            let publisher = app_key
                .get_value::<String, _>("Publisher")
                .unwrap_or_default();

            apps.push(AppInfo {
                name,
                version,
                publisher,
            });
        }
    }

    // 1. First sort by name, then by version (descending).
    apps.sort_unstable_by(|a, b| a.name.cmp(&b.name).then_with(|| b.version.cmp(&a.version)));

    // 2. Deduplicate by name, keeping the highest version due to the sort order.
    apps.dedup_by(|a, b| a.name == b.name);

    Ok(apps)
}
