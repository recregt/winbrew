use super::Result;
use super::state;
use super::{InstallObserver, ResolvedInstallTarget, resolve_install_target, sevenz};
use crate::models::domains::package::PackageRef;
use url::Url;

/// A read-only install preview.
pub struct InstallPreview {
    target: ResolvedInstallTarget,
    inspection: state::InstallTargetInspection,
    ignore_checksum_security: bool,
}

/// Build a read-only preview of the install operation.
pub fn build_install_preview<O: InstallObserver>(
    ctx: &crate::AppContext,
    package_ref: PackageRef,
    ignore_checksum_security: bool,
    observer: &mut O,
) -> Result<InstallPreview> {
    let target = resolve_install_target(ctx, package_ref, |query, matches| {
        observer.choose_package(query, matches)
    })?;
    let conn = crate::database::get_conn()?;
    let inspection = state::inspect_install_target_with_commands(
        &conn,
        &target.package.name,
        &target.install_dir,
        target.resolved_commands_json.as_deref(),
    )?;

    Ok(InstallPreview {
        target,
        inspection,
        ignore_checksum_security,
    })
}

/// Return the human-readable lines that describe the preview.
pub fn preview_lines(
    ctx: &crate::AppContext,
    preview: &InstallPreview,
    show_temp_root: bool,
) -> Vec<String> {
    let mut lines = Vec::new();

    lines.push(format!(
        "Package: {} {}",
        preview.target.package.name, preview.target.package.version
    ));
    lines.push(format!(
        "Installer URL: {}",
        shorten_url(&preview.target.installer.url)
    ));
    lines.push(format!(
        "Download payload: {}",
        match preview
            .target
            .download_path
            .file_name()
            .and_then(|value| value.to_str())
        {
            Some(file_name) => file_name.to_string(),
            None => preview.target.download_path.display().to_string(),
        }
    ));
    let engine = preview.target.manifest_engine.as_str();
    let deployment_kind = preview.target.manifest_deployment_kind.as_str();
    lines.push(format!("Engine: {engine}"));
    if engine != deployment_kind {
        lines.push(format!("Deployment: {deployment_kind}"));
    }
    lines.push(format!(
        "Install dir: {}",
        preview.target.install_dir.display()
    ));
    if show_temp_root {
        lines.push(format!("Temp root: {}", preview.target.temp_root.display()));
    }
    lines.push(format!(
        "Checksum policy: {}",
        if preview.ignore_checksum_security {
            "legacy algorithms allowed"
        } else {
            "strict"
        }
    ));

    match preview.target.resolved_commands.as_deref() {
        Some(commands) if !commands.is_empty() => {
            lines.push(format!("Command shims: {}", commands.join(", ")));
        }
        _ => {
            lines.push("Command shims: none".to_string());
        }
    }

    if preview.target.runtime_bootstrap_required {
        lines.push(format!(
            "7-Zip runtime bootstrap: required for {}",
            sevenz::sevenz_runtime_dir_from_runtime_root(&ctx.paths.root).display()
        ));
    } else {
        lines.push("7-Zip runtime bootstrap: not required".to_string());
    }

    match preview.inspection.state {
        state::InstallTargetState::Ready => {
            lines.push("Preflight: no blockers found".to_string());
        }
        state::InstallTargetState::AlreadyInstalled => {
            lines.push("Preflight blocker: package is already installed".to_string());
        }
        state::InstallTargetState::AlreadyInstalling => {
            lines.push("Preflight blocker: package is already installing".to_string());
        }
        state::InstallTargetState::CurrentlyUpdating => {
            lines.push("Preflight blocker: package is currently updating".to_string());
        }
        state::InstallTargetState::Failed => {
            lines.push("Preflight: stale failed record will be cleaned up".to_string());
        }
        state::InstallTargetState::Orphaned => {
            lines.push("Preflight: orphaned install directory will be cleaned up".to_string());
        }
    }

    for conflict in &preview.inspection.command_conflicts {
        lines.push(format!(
            "Preflight blocker: command '{}' is already exposed by package '{}'",
            conflict.command, conflict.package
        ));
    }

    lines
}

fn shorten_url(raw_url: &str) -> String {
    let Ok(parsed_url) = Url::parse(raw_url) else {
        return raw_url.to_string();
    };

    let Some(host) = parsed_url.host_str() else {
        return raw_url.to_string();
    };

    let segments = parsed_url
        .path_segments()
        .map(|segments| {
            segments
                .filter(|segment| !segment.is_empty())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    match segments.as_slice() {
        [] => host.to_string(),
        [only] => format!("{host}/{only}"),
        [.., last] => format!("{host}/.../{last}"),
    }
}
