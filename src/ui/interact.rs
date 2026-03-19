use super::Ui;
use anyhow::Result;
use console::Style;
use dialoguer::{Confirm, theme::ColorfulTheme};
use std::io::Write;

impl<W: Write> Ui<W> {
    pub fn page_title(&mut self, title: &str) {
        if self.color_enabled {
            let arrow = Style::new().cyan().bold().apply_to("==>");
            let text = Style::new().bold().apply_to(title);
            let _ = writeln!(self.err, "{arrow} {text}");
        } else {
            let _ = writeln!(self.err, "==> {title}");
        }
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
