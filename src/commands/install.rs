use anyhow::Result;

use crate::{services::install, ui::Ui};

pub fn run(query: &[String], version: Option<&str>) -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("Install");

    let pb = ui.progress_bar();
    let resolved = install::resolve::resolve(query, version, &mut ui)?;

    install::install(
        &resolved.identifier,
        &resolved.version,
        |downloaded, total| {
            pb.set_length(total);
            pb.set_position(downloaded);
        },
    )?;

    pb.finish_and_clear();
    ui.success(format!(
        "{}@{} is ready.",
        resolved.identifier, resolved.version
    ));

    Ok(())
}
