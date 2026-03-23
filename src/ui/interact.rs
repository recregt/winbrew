use super::Ui;
use anyhow::Result;
use dialoguer::{Confirm, theme::ColorfulTheme};
use std::io::Write;

impl<W: Write> Ui<W> {
    pub fn page_title(&mut self, title: &str) {
        let _ = title;
    }

    pub fn confirm(&mut self, message: &str, default: bool) -> Result<bool> {
        if self.default_yes {
            return Ok(true);
        }

        Confirm::with_theme(&ColorfulTheme::default())
            .with_prompt(message)
            .default(default)
            .interact()
            .map_err(Into::into)
    }
}
