use anyhow::Result;
use std::io::Write;

use crate::CommandContext;
use crate::app::doctor;
use crate::app::install::InstallObserver;
use crate::app::models::CatalogPackage;
use crate::app::repair::{self, FileRestoreResolution, RepairPlan};
use winbrew_ui::Ui;

pub fn run(ctx: &CommandContext, yes: bool) -> Result<()> {
    let mut ui = ctx.ui();
    ui.page_title("Repair");

    let report = ui.spinner("Inspecting recovery findings...", || {
        doctor::health_report(ctx)
    })?;
    let plan = repair::build_repair_plan(&report, &ctx.paths.packages);

    if plan.is_empty() {
        ui.success("No supported recovery actions were found.");
        if plan.file_restore_count > 0 || plan.reinstall_count > 0 {
            ui.warn(format!(
                "Found {} file restore and {} reinstall finding(s), but no package targets were derived.",
                plan.file_restore_count, plan.reinstall_count
            ));
        }
        return Ok(());
    }

    let mut applied = 0usize;

    applied += run_journal_replay_group(&mut ui, yes, &plan)?;
    applied += run_orphan_cleanup_group(&mut ui, yes, &plan)?;
    applied += run_file_restore_group(&mut ui, ctx, &plan)?;
    applied += run_reinstall_group(&mut ui, ctx, &plan.reinstall_packages)?;

    if applied == 0 {
        ui.notice("No recovery actions were applied.");
    }

    Ok(())
}

fn run_journal_replay_group<W: Write>(
    ui: &mut Ui<W>,
    yes: bool,
    plan: &RepairPlan,
) -> Result<usize> {
    if plan.journal_paths.is_empty() {
        return Ok(0);
    }

    ui.info(format!(
        "Found {} committed journal replay candidate(s).",
        plan.journal_paths.len()
    ));

    if !confirm_group(
        ui,
        yes,
        true,
        &format!(
            "Replay {} committed journal(s) into SQLite?",
            plan.journal_paths.len()
        ),
        "Skipped journal replay.",
    )? {
        return Ok(0);
    }

    let replayed = ui.spinner(
        format!(
            "Replaying {} committed journal(s)...",
            plan.journal_paths.len()
        ),
        || repair::replay_committed_journals(&plan.journal_paths),
    )?;

    ui.success(format!("Replayed {replayed} committed journal(s)."));
    Ok(replayed)
}

fn run_orphan_cleanup_group<W: Write>(
    ui: &mut Ui<W>,
    yes: bool,
    plan: &RepairPlan,
) -> Result<usize> {
    if plan.orphan_paths.is_empty() {
        return Ok(0);
    }

    ui.info(format!(
        "Found {} orphan install directory candidate(s).",
        plan.orphan_paths.len()
    ));

    if !confirm_group(
        ui,
        yes,
        true,
        &format!(
            "Remove {} orphan install director{}?",
            plan.orphan_paths.len(),
            if plan.orphan_paths.len() == 1 {
                "y"
            } else {
                "ies"
            }
        ),
        "Skipped orphan cleanup.",
    )? {
        return Ok(0);
    }

    let removed = ui.spinner(
        format!(
            "Removing {} orphan install director{}...",
            plan.orphan_paths.len(),
            if plan.orphan_paths.len() == 1 {
                "y"
            } else {
                "ies"
            }
        ),
        || repair::cleanup_orphan_install_dirs(&plan.orphan_paths),
    )?;

    ui.success(format!(
        "Removed {removed} orphan install director{}.",
        if removed == 1 { "y" } else { "ies" }
    ));
    Ok(removed)
}

fn run_file_restore_group<W: Write>(
    ui: &mut Ui<W>,
    ctx: &CommandContext,
    plan: &RepairPlan,
) -> Result<usize> {
    if plan.file_restore_packages.is_empty() {
        return Ok(0);
    }

    ui.info(format!(
        "Found {} file restore package candidate(s).",
        plan.file_restore_packages.len()
    ));

    let mut repaired = 0usize;

    for package_target in &plan.file_restore_packages {
        let target_count = package_target.target_paths.len();

        if !confirm_group(
            ui,
            false,
            false,
            &format!(
                "Restore {} file{} for {}?",
                target_count,
                if target_count == 1 { "" } else { "s" },
                package_target.name
            ),
            &format!("Skipped file restore for {}.", package_target.name),
        )? {
            continue;
        }

        let resolution =
            repair::resolve_file_restore_target(&package_target.name, |query, matches| {
                choose_catalog_package(ui, query, matches)
            })?;

        match resolution {
            FileRestoreResolution::Restore(target) => {
                let restored = ui.spinner(
                    format!(
                        "Restoring {} file{} for {}...",
                        target_count,
                        if target_count == 1 { "" } else { "s" },
                        package_target.name
                    ),
                    || repair::restore_file_restore_target(&target, &package_target.target_paths),
                )?;

                ui.success(format!(
                    "Restored {} file{} for {}.",
                    restored,
                    if restored == 1 { "" } else { "s" },
                    package_target.name
                ));
                repaired += 1;
            }
            FileRestoreResolution::Reinstall(target) => {
                ui.notice(format!(
                    "Catalog version {} differs from installed version {}; reinstalling {} instead.",
                    target.catalog_package.version,
                    target.installed_version,
                    package_target.name
                ));

                if !confirm_group(
                    ui,
                    false,
                    false,
                    &format!("Reinstall {} instead?", package_target.name),
                    &format!("Skipped reinstall fallback for {}.", package_target.name),
                )? {
                    continue;
                }

                let outcome = ui.spinner(
                    format!("Reinstalling {}...", target.catalog_package.name),
                    || {
                        let mut observer = NoopInstallObserver;
                        repair::reinstall_package(ctx, &target.catalog_package, &mut observer)
                    },
                )?;

                ui.success(format!(
                    "Repaired {} {}.",
                    outcome.result.name, outcome.result.version
                ));
                repaired += 1;
            }
        }
    }

    Ok(repaired)
}

fn run_reinstall_group<W: Write>(
    ui: &mut Ui<W>,
    ctx: &CommandContext,
    package_names: &[String],
) -> Result<usize> {
    if package_names.is_empty() {
        return Ok(0);
    }

    ui.info(format!(
        "Found {} reinstall package candidate(s).",
        package_names.len()
    ));

    let mut repaired = 0usize;

    for package_name in package_names {
        if !confirm_group(
            ui,
            false,
            false,
            &format!("Reinstall {package_name}?"),
            &format!("Skipped reinstall for {package_name}."),
        )? {
            continue;
        }

        let catalog_package =
            repair::resolve_repair_catalog_package(package_name, |query, matches| {
                choose_catalog_package(ui, query, matches)
            })?;

        let outcome = ui.spinner(format!("Reinstalling {}...", catalog_package.name), || {
            let mut observer = NoopInstallObserver;
            repair::reinstall_package(ctx, &catalog_package, &mut observer)
        })?;

        ui.success(format!(
            "Repaired {} {}.",
            outcome.result.name, outcome.result.version
        ));
        repaired += 1;
    }

    Ok(repaired)
}

fn confirm_group<W: Write>(
    ui: &mut Ui<W>,
    yes: bool,
    allow_auto_yes: bool,
    prompt: &str,
    skipped_message: &str,
) -> Result<bool> {
    if allow_auto_yes && yes {
        return Ok(true);
    }

    if ui.confirm(prompt, false)? {
        return Ok(true);
    }

    ui.notice(skipped_message);
    Ok(false)
}

fn choose_catalog_package<W: Write>(
    ui: &mut Ui<W>,
    query: &str,
    matches: &[CatalogPackage],
) -> Result<usize> {
    let choices = matches
        .iter()
        .map(format_catalog_choice)
        .collect::<Vec<_>>();

    ui.select_index(
        &format!("Multiple packages matched '{query}'. Choose one:"),
        &choices,
    )
}

fn format_catalog_choice(pkg: &CatalogPackage) -> String {
    let mut label = String::with_capacity(128);
    label.push_str(&pkg.name);
    label.push(' ');
    label.push_str(&pkg.version.to_string());

    if let Some(publisher) = pkg
        .publisher
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        label.push_str(" - ");
        label.push_str(publisher);
    }

    if let Some(description) = pkg
        .description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        label.push_str(" (");
        label.push_str(description);
        label.push(')');
    }

    label
}

struct NoopInstallObserver;

impl InstallObserver for NoopInstallObserver {
    fn choose_package(
        &mut self,
        query: &str,
        _matches: &[CatalogPackage],
    ) -> anyhow::Result<usize> {
        unreachable!("install should not prompt for package selection for '{query}'")
    }

    fn on_start(&mut self, _total_bytes: Option<u64>) {}

    fn on_progress(&mut self, _downloaded_bytes: u64) {}
}
