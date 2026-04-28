use winreg::{RegKey, enums::HKEY_LOCAL_MACHINE};

const WINDOWS_CURRENT_VERSION_KEY: &str = r"SOFTWARE\Microsoft\Windows NT\CurrentVersion";
const CURRENT_MAJOR_VERSION_NUMBER: &str = "CurrentMajorVersionNumber";
const CURRENT_MINOR_VERSION_NUMBER: &str = "CurrentMinorVersionNumber";
const CURRENT_VERSION: &str = "CurrentVersion";
const CURRENT_BUILD_NUMBER: &str = "CurrentBuildNumber";
const CURRENT_BUILD: &str = "CurrentBuild";
const UBR: &str = "UBR";

/// Return the current Windows version string when the registry exposes it.
pub fn windows_version_string() -> Option<String> {
    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    let Ok(current_version_key) = hklm.open_subkey(WINDOWS_CURRENT_VERSION_KEY) else {
        return None;
    };

    let major = current_version_key
        .get_value::<u32, _>(CURRENT_MAJOR_VERSION_NUMBER)
        .ok();
    let minor = current_version_key
        .get_value::<u32, _>(CURRENT_MINOR_VERSION_NUMBER)
        .ok();
    let prefix = match (major, minor) {
        (Some(major), Some(minor)) => format!("{major}.{minor}"),
        _ => current_version_key
            .get_value::<String, _>(CURRENT_VERSION)
            .ok()?
            .trim()
            .to_string(),
    };

    let build = current_version_key
        .get_value::<String, _>(CURRENT_BUILD_NUMBER)
        .ok()
        .or_else(|| {
            current_version_key
                .get_value::<String, _>(CURRENT_BUILD)
                .ok()
        })?;

    let ubr = current_version_key.get_value::<u32, _>(UBR).ok();

    format_windows_version(&prefix, &build, ubr)
}

fn format_windows_version(prefix: &str, build: &str, ubr: Option<u32>) -> Option<String> {
    let prefix = prefix.trim();
    let build = build.trim();

    if prefix.is_empty() || build.is_empty() {
        return None;
    }

    let mut version = format!("{prefix}.{build}");
    if let Some(ubr) = ubr {
        version.push('.');
        version.push_str(&ubr.to_string());
    }

    Some(version)
}

#[cfg(test)]
mod tests {
    use super::format_windows_version;

    #[test]
    fn formats_windows_version_strings() {
        assert_eq!(
            format_windows_version("10.0", "26200", Some(8246)),
            Some("10.0.26200.8246".to_string())
        );
        assert_eq!(
            format_windows_version("6.3", "9600", None),
            Some("6.3.9600".to_string())
        );
        assert_eq!(format_windows_version("", "9600", Some(1)), None);
        assert_eq!(format_windows_version("10.0", "", Some(1)), None);
    }
}
