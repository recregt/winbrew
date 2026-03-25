use crate::manifest::InstallerEntry;
use std::cmp::Ordering;

pub fn select_installer(installers: &[InstallerEntry]) -> Option<&InstallerEntry> {
    select_installer_with_context(installers, &SelectionContext::current())
}

pub fn select_installer_with_context<'a>(
    installers: &'a [InstallerEntry],
    context: &SelectionContext,
) -> Option<&'a InstallerEntry> {
    installers
        .iter()
        .enumerate()
        .max_by(|(left_index, left), (right_index, right)| {
            compare(left, right, context).then_with(|| right_index.cmp(left_index))
        })
        .map(|(_, installer)| installer)
}

#[derive(Debug, Clone)]
pub struct SelectionContext {
    system_architecture: String,
}

impl SelectionContext {
    pub fn current() -> Self {
        Self {
            system_architecture: normalize_system_architecture(std::env::consts::ARCH),
        }
    }

    pub fn for_architecture(system_architecture: &str) -> Self {
        Self {
            system_architecture: normalize_system_architecture(system_architecture),
        }
    }

    fn system_architecture(&self) -> &str {
        self.system_architecture.as_str()
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
struct Score {
    architecture: i32,
    scope: i32,
    installer_type: i32,
}

fn compare(left: &InstallerEntry, right: &InstallerEntry, context: &SelectionContext) -> Ordering {
    compare_score(&score(left, context), &score(right, context))
}

fn compare_score(left: &Score, right: &Score) -> Ordering {
    (left.architecture, left.scope, left.installer_type).cmp(&(
        right.architecture,
        right.scope,
        right.installer_type,
    ))
}

fn score(installer: &InstallerEntry, context: &SelectionContext) -> Score {
    let architecture_name = normalize_value(&installer.architecture);
    let installer_type_name = normalize_value(&installer.installer_type);
    let scope_name = normalize_value(installer.scope.as_deref().unwrap_or_default());

    let architecture =
        architecture_score(architecture_name.as_str(), context.system_architecture());
    let scope = scope_score(scope_name.as_str());
    let installer_type = installer_type_score(installer_type_name.as_str());

    Score {
        architecture,
        scope,
        installer_type,
    }
}

fn architecture_score(installer_architecture: &str, system_architecture: &str) -> i32 {
    match (system_architecture, installer_architecture) {
        (system, installer) if system == installer => 1000,
        ("x64", "x86") => 500,
        ("arm64", "x64") => 500,
        ("arm64", "x86") => 250,
        (_, "neutral") | (_, "any") | (_, "all") => 300,
        (_, _) => -10_000,
    }
}

fn scope_score(scope: &str) -> i32 {
    match scope {
        // Prefer user scope by default to avoid elevation and keep installs per-user.
        "user" => 100,
        "machine" => 50,
        _ => 0,
    }
}

fn installer_type_score(installer_type: &str) -> i32 {
    match installer_type {
        "msi" => 50,
        "msix" => 45,
        "msixbundle" => 44,
        "appx" => 43,
        "exe" => 20,
        "inno" => 18,
        "nullsoft" | "nsis" => 17,
        "wix" => 16,
        "zip" => 10,
        "portable" => 9,
        _ => 0,
    }
}

fn normalize_system_architecture(architecture: &str) -> String {
    match architecture {
        "x86_64" => "x64".to_string(),
        "x86" => "x86".to_string(),
        "aarch64" => "arm64".to_string(),
        other => other.to_ascii_lowercase(),
    }
}

fn normalize_value(value: &str) -> String {
    value.trim().to_ascii_lowercase()
}
