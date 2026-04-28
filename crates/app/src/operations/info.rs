use anyhow::Result;

use crate::core::paths::ResolvedPaths;
use crate::models::domains::install::Architecture;
use crate::models::domains::reporting::InfoReport;
use crate::models::domains::shared::ConfigSection;
use crate::report;
use crate::version;

pub fn collect(sections: &[ConfigSection], resolved_paths: &ResolvedPaths) -> Result<InfoReport> {
    Ok(InfoReport {
        version: version::package_version().to_string(),
        system: system_entries(),
        runtime: report::runtime_report(sections, resolved_paths)?,
    })
}

fn system_entries() -> Vec<(String, String)> {
    let host_profile = crate::windows::host_profile();
    let family = if host_profile.is_server {
        "Windows.Server"
    } else {
        "Windows.Desktop"
    };

    let windows_label = crate::windows::windows_version_string()
        .map(|version| format!("{family} v{version}"))
        .unwrap_or_else(|| family.to_string());

    vec![
        ("Windows".to_string(), windows_label),
        (
            "System Architecture".to_string(),
            architecture_label(host_profile.architecture).to_string(),
        ),
    ]
}

fn architecture_label(architecture: Architecture) -> &'static str {
    match architecture {
        Architecture::X64 => "X64",
        Architecture::X86 => "X86",
        Architecture::Arm64 => "ARM64",
        Architecture::Any => "Unknown",
    }
}
