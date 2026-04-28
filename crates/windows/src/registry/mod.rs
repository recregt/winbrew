use anyhow::Result;
use regex::RegexBuilder;

mod product_options;
mod test_support;
mod uninstall;
pub(crate) mod user_fonts;
mod windows_version;

pub(crate) use product_options::read_product_type;
pub use test_support::{
    UninstallEntryGuard, create_test_uninstall_entry,
    create_test_uninstall_entry_with_install_location,
};
use uninstall::uninstall_roots;
pub(crate) use user_fonts::{register_user_font_value, unregister_user_font_value};
pub use windows_version::windows_version_string;

/// Snapshot of one uninstall registry entry.
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct UninstallEntry {
    /// Application display name.
    pub display_name: String,
    /// Application version string, if the registry entry exposes one.
    pub version: String,
    /// Publisher string, if the registry entry exposes one.
    pub publisher: String,
    /// Install location stored in the registry, if present.
    pub install_location: Option<String>,
    /// Quiet uninstall command stored in the registry, if present.
    pub quiet_uninstall_string: Option<String>,
    /// Standard uninstall command stored in the registry, if present.
    pub uninstall_string: Option<String>,
}

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

/// Collect installed applications from the available uninstall registry entries.
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
    let mut apps = Vec::new();

    visit_uninstall_entries(filter, |entry| {
        apps.push(AppInfo {
            name: entry.display_name,
            version: entry.version,
            publisher: entry.publisher,
        });
    })?;

    // 1. First sort by name, then by version (descending).
    apps.sort_unstable_by(|a, b| a.name.cmp(&b.name).then_with(|| b.version.cmp(&a.version)));

    // 2. Deduplicate by name, keeping the highest version due to the sort order.
    apps.dedup_by(|a, b| a.name == b.name);

    Ok(apps)
}

/// Collect uninstall registry entries that match the optional display-name filter.
///
/// The optional `filter` is treated as a case-insensitive literal search on the
/// entry display name. Missing values are normalized to `None` or empty strings
/// so callers can work with plain Rust types instead of registry handles.
pub fn collect_uninstall_entries(filter: Option<&str>) -> Result<Vec<UninstallEntry>> {
    let mut entries = Vec::new();

    visit_uninstall_entries(filter, |entry| entries.push(entry))?;

    Ok(entries)
}

fn visit_uninstall_entries<F>(filter: Option<&str>, mut visit: F) -> Result<()>
where
    F: FnMut(UninstallEntry),
{
    let pattern = filter
        .map(|f| {
            RegexBuilder::new(&regex::escape(f))
                .case_insensitive(true)
                .build()
        })
        .transpose()?;

    for root in uninstall_roots() {
        for key_result in root.key().enum_keys() {
            let Ok(key_name) = key_result else { continue };
            let Ok(app_key) = root.key().open_subkey(&key_name) else {
                continue;
            };

            let Ok(display_name) = app_key.get_value::<String, _>("DisplayName") else {
                continue;
            };

            if pattern
                .as_ref()
                .is_some_and(|re| !re.is_match(&display_name))
            {
                continue;
            }

            visit(UninstallEntry {
                display_name,
                version: app_key
                    .get_value::<String, _>("DisplayVersion")
                    .unwrap_or_default(),
                publisher: app_key
                    .get_value::<String, _>("Publisher")
                    .unwrap_or_default(),
                install_location: read_optional_string(&app_key, "InstallLocation"),
                quiet_uninstall_string: read_optional_string(&app_key, "QuietUninstallString"),
                uninstall_string: read_optional_string(&app_key, "UninstallString"),
            });
        }
    }

    Ok(())
}

/// Read the first non-empty string value from an uninstall entry identified by key name.
///
/// MSI install flows use this to read `InstallLocation` after `msiexec`
/// completes so the engine can store the final path reported by Windows.
pub fn uninstall_value(key_name: &str, value_name: &str) -> Option<String> {
    for root in uninstall_roots() {
        let Ok(app_key) = root.key().open_subkey(key_name) else {
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

fn read_optional_string(app_key: &winreg::RegKey, value_name: &str) -> Option<String> {
    let Ok(value) = app_key.get_value::<String, _>(value_name) else {
        return None;
    };

    let value = value.trim();

    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::{collect_installed_apps, collect_uninstall_entries};
    use crate::registry::create_test_uninstall_entry_with_install_location;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_install_dir(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "winbrew-registry-helper-{}-{}-{name}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should be monotonic")
                .as_nanos()
        ))
    }

    #[test]
    fn collects_uninstall_entries_and_projects_them_to_apps() {
        let package_name = "WinBrew Registry Helper";
        let install_dir = temp_install_dir("registry-helper");
        let _guard = create_test_uninstall_entry_with_install_location(
            package_name,
            Some(&install_dir),
            Some("/quiet"),
            Some("/uninstall"),
        )
        .expect("test uninstall entry should be created");

        let entries = collect_uninstall_entries(Some(package_name))
            .expect("uninstall entries should be collected");

        assert_eq!(entries.len(), 1);
        let entry = &entries[0];
        assert_eq!(entry.display_name, package_name);
        assert_eq!(
            entry.install_location.as_deref(),
            Some(install_dir.to_string_lossy().as_ref())
        );
        assert_eq!(entry.quiet_uninstall_string.as_deref(), Some("/quiet"));
        assert_eq!(entry.uninstall_string.as_deref(), Some("/uninstall"));

        let apps = collect_installed_apps(Some(package_name)).expect("apps should be collected");
        assert_eq!(apps.len(), 1);
        assert_eq!(apps[0].name, package_name);
    }
}
