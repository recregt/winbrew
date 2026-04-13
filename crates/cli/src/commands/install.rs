use anyhow::Result;
use indicatif::ProgressBar;
use std::io;

use crate::CommandContext;
use crate::app::install;
use crate::app::install::InstallError;
use crate::app::install::InstallObserver;
use crate::commands::error::{cancelled, reported_with_hint};
use crate::models::domains::catalog::CatalogPackage;
use crate::models::domains::package::PackageRef;
use winbrew_ui::Ui;

pub fn run(ctx: &CommandContext, query: &[String], ignore_checksum_security: bool) -> Result<()> {
    let mut ui = ctx.ui();
    ui.page_title("Install Package");

    let query_text = query.join(" ").trim().to_owned();
    if query_text.is_empty() {
        return Err(anyhow::Error::msg("package query cannot be empty"));
    }

    let package_ref = PackageRef::parse(&query_text).map_err(anyhow::Error::msg)?;

    ui.info(format!("Resolving {query_text}..."));

    let progress = ui.progress_bar();

    let result = {
        let mut observer = InstallUi {
            ui: &mut ui,
            progress: &progress,
        };

        install::run(
            ctx.app(),
            package_ref,
            ignore_checksum_security,
            &mut observer,
        )
    };

    progress.finish_and_clear();

    match result {
        Ok(outcome) => {
            for algorithm in outcome.legacy_checksum_algorithms {
                ui.warn(format!(
                    "This package uses {} checksums. Verification succeeded, but {} is a legacy algorithm.",
                    algorithm.display_name(),
                    algorithm.display_name()
                ));
            }

            let result = outcome.result;
            ui.success(format!(
                "Installed {} {} into {}.",
                result.name, result.version, result.install_dir
            ));
        }
        Err(err) => match err {
            InstallError::AlreadyInstalled { name } => {
                ui.notice(format!("{name} is already installed."));
            }
            InstallError::AlreadyInstalling { name } => {
                ui.warn(format!("{name} is currently being installed."));
            }
            InstallError::CurrentlyUpdating { name } => {
                ui.warn(format!("{name} is currently updating."));
            }
            InstallError::ChecksumMismatch { expected, actual } => {
                let message =
                    format!("Installer checksum mismatch: expected {expected}, got {actual}");
                ui.error(&message);
                ui.notice(
                    "Hint: re-download the installer or refresh the catalog before retrying.",
                );
                return Err(reported_with_hint(
                    message,
                    "Re-download the installer or refresh the catalog before retrying.",
                ));
            }
            InstallError::LegacyChecksumAlgorithm { algorithm } => {
                let message = format!(
                    "{} checksums are disabled by default for security. Re-run with --ignore-checksum-security to install this package.",
                    algorithm.display_name()
                );
                ui.error(&message);
                ui.notice("Hint: re-run with --ignore-checksum-security only if you trust the package source.");
                return Err(reported_with_hint(
                    message,
                    "Re-run with --ignore-checksum-security only if you trust the package source.",
                ));
            }
            InstallError::Cancelled => {
                ui.notice("Cancelling and cleaning up...");
                return Err(cancelled());
            }
            InstallError::Unexpected(err) => {
                return Err(err);
            }
        },
    }

    Ok(())
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
        .filter(|value: &&str| !value.is_empty())
    {
        label.push_str(" - ");
        label.push_str(publisher);
    }

    if let Some(description) = pkg
        .description
        .as_deref()
        .map(str::trim)
        .filter(|value: &&str| !value.is_empty())
    {
        label.push_str(" (");
        label.push_str(description);
        label.push(')');
    }

    label
}

struct InstallUi<'a> {
    ui: &'a mut Ui<io::Stdout>,
    progress: &'a ProgressBar,
}

impl InstallObserver for InstallUi<'_> {
    fn choose_package(&mut self, query: &str, matches: &[CatalogPackage]) -> anyhow::Result<usize> {
        let choices = matches
            .iter()
            .map(format_catalog_choice)
            .collect::<Vec<_>>();

        self.ui.select_index(
            &format!("Multiple packages matched '{query}'. Choose one:"),
            &choices,
        )
    }

    fn on_start(&mut self, total_bytes: Option<u64>) {
        if let Some(total_bytes) = total_bytes {
            self.progress.set_length(total_bytes);
        }
        self.progress.set_message("Downloading installer");
    }

    fn on_progress(&mut self, downloaded_bytes: u64) {
        self.progress.inc(downloaded_bytes);
    }
}
