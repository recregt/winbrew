use anyhow::Result;

use crate::{services::install, ui::Ui};

pub fn run(name: &str, version: &str) -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("Install");

    let pb = ui.progress_bar();

    install::install(name, version, |downloaded, total| {
        pb.set_length(total);
        pb.set_position(downloaded);
    })?;

    pb.finish_and_clear();
    ui.success(format!("{name}@{version} is ready."));

    Ok(())
}
