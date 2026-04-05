use anyhow::Result;

use crate::models::CatalogPackage;
use crate::services::install;
use crate::{AppContext, ui::Ui};

pub fn run(ctx: &AppContext, query: &[String], ignore_checksum_security: bool) -> Result<()> {
    let mut ui = Ui::new(ctx.ui);
    ui.page_title("Install Package");

    let query_text = query.join(" ");
    ui.info(format!("Resolving {query_text}..."));

    let progress = ui.progress_bar();

    let result = install::run(
        ctx,
        query,
        ignore_checksum_security,
        |query, matches| {
            let choices = matches
                .iter()
                .map(format_catalog_choice)
                .collect::<Vec<_>>();

            ui.select_index(
                &format!("Multiple packages matched '{query}'. Choose one:"),
                &choices,
            )
        },
        |total_bytes| {
            if let Some(total_bytes) = total_bytes {
                progress.set_length(total_bytes);
            }
            progress.set_message("Downloading installer");
        },
        |downloaded_bytes| {
            progress.inc(downloaded_bytes);
        },
    );

    progress.finish_and_clear();

    let result = result?;
    ui.success(format!(
        "Installed {} {} into {}.",
        result.name, result.version, result.install_dir
    ));

    Ok(())
}

fn format_catalog_choice(pkg: &CatalogPackage) -> String {
    let mut label = String::with_capacity(128);
    label.push_str(&pkg.name);
    label.push(' ');
    label.push_str(&pkg.version);

    if let Some(publisher) = pkg
        .publisher
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        label.push_str(" - ");
        label.push_str(publisher);
    }

    if let Some(description) = pkg
        .description
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        label.push_str(" (");
        label.push_str(description);
        label.push(')');
    }

    label
}
