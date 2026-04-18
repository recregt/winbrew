use anyhow::Result;
use regex::RegexBuilder;

mod product_options;
mod test_support;
mod uninstall;
pub(crate) mod user_fonts;

pub(crate) use product_options::read_product_type;
pub use test_support::{
    UninstallEntryGuard, create_test_uninstall_entry,
    create_test_uninstall_entry_with_install_location,
};
pub use uninstall::{Hive, UninstallRoot, uninstall_roots};
pub(crate) use user_fonts::{register_user_font_value, unregister_user_font_value};

/// Display information collected from uninstall registry entries.
#[derive(Debug, Eq, PartialEq)]
pub struct AppInfo {
    /// Application display name.
    pub name: String,
    /// Application version string, if the registry entry exposes one.
    pub version: String,
    /// Publisher string, if the registry entry exposes one.
    pub publisher: String,
}

/// Collect installed applications from the available uninstall registry roots.
///
/// The optional `filter` is treated as a case-insensitive literal search. Any
/// regex metacharacters are escaped before matching, so the caller can pass a
/// human-friendly package name instead of a regex.
///
/// Results are sorted by name first and then by version in descending
/// lexicographic order. After sorting, entries with the same name are removed so
/// the first entry for each name wins. That keeps the highest version encountered
/// for each application name, which is good enough for display and removal
/// workflows, but it is not a semantic-version comparison.
///
/// # Example
///
/// ```no_run
/// use winbrew_windows::collect_installed_apps;
///
/// let apps = collect_installed_apps(Some("winbrew")).unwrap();
/// for app in apps {
///     println!("{} {} - {}", app.name, app.version, app.publisher);
/// }
/// ```
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

/// Read the first non-empty string value from an uninstall entry identified by key name.
///
/// MSI install flows use this to read `InstallLocation` after `msiexec`
/// completes so the engine can store the final path reported by Windows.
pub fn uninstall_value(key_name: &str, value_name: &str) -> Option<String> {
    for root in uninstall_roots() {
        let Ok(app_key) = root.key.open_subkey(key_name) else {
            continue;
        };

        let Ok(value) = app_key.get_value::<String, _>(value_name) else {
            continue;
        };

        if !value.trim().is_empty() {
            return Some(value);
        }
    }

    None
}
