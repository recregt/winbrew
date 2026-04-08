use super::Ui;
use anyhow::{Result, bail};
use dialoguer::{Confirm, Input, Select, theme::ColorfulTheme};
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

    pub fn prompt_text(&mut self, message: &str, default: Option<&str>) -> Result<String> {
        let theme = ColorfulTheme::default();
        let input = Input::<String>::with_theme(&theme).with_prompt(message);
        let input = if let Some(default) = default {
            input.default(default.to_string())
        } else {
            input
        };

        input.interact_text().map_err(Into::into)
    }

    pub fn prompt_number(&mut self, message: &str, max: usize) -> Result<usize> {
        if max == 0 {
            bail!("cannot prompt for selection from an empty list");
        }

        loop {
            let value = self.prompt_text(message, None)?;
            match value.trim().parse::<usize>() {
                Ok(number) if (1..=max).contains(&number) => return Ok(number - 1),
                _ => self.warn(format!("Enter a number between 1 and {max}.")),
            }
        }
    }

    pub fn select_index(&mut self, message: &str, items: &[String]) -> Result<usize> {
        if items.is_empty() {
            bail!("cannot prompt for selection from an empty list");
        }

        Select::with_theme(&ColorfulTheme::default())
            .with_prompt(message)
            .items(items)
            .default(0)
            .interact()
            .map_err(Into::into)
    }
}
