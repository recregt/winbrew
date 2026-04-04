use super::Ui;
use crate::ui::current_settings;
use crate::ui::theme;
use std::io::{self, BufWriter, Write};
use tracing::warn;

#[derive(Default)]
pub(crate) struct ConfigOverrides {
    pub color: Option<bool>,
    pub default_yes: Option<bool>,
}

pub struct UiBuilder<W: Write> {
    out: W,
    err: Box<dyn Write>,
    color_enabled: Option<bool>,
    default_yes: Option<bool>,
    config_overrides: ConfigOverrides,
}

impl UiBuilder<io::Stdout> {
    pub fn new() -> Self {
        UiBuilder {
            out: io::stdout(),
            err: Box::new(io::stderr()),
            color_enabled: None,
            default_yes: None,
            config_overrides: ConfigOverrides::default(),
        }
    }
}

impl Default for UiBuilder<io::Stdout> {
    fn default() -> Self {
        Self::new()
    }
}

impl<W: Write> UiBuilder<W> {
    pub fn with_writer(out: W) -> Self {
        UiBuilder {
            out,
            err: Box::new(io::stderr()),
            color_enabled: None,
            default_yes: None,
            config_overrides: ConfigOverrides::default(),
        }
    }

    pub fn with_error_writer(mut self, err: Box<dyn Write>) -> Self {
        self.err = err;
        self
    }

    pub fn with_config(mut self, key: &str, value: bool) -> Self {
        match key {
            "color" => self.config_overrides.color = Some(value),
            "default_yes" => self.config_overrides.default_yes = Some(value),
            _ => {
                warn!("unknown config key: {key}");
            }
        }
        self
    }

    pub fn color_enabled(mut self, color: bool) -> Self {
        self.color_enabled = Some(color);
        self
    }

    pub fn default_yes(mut self, default_yes: bool) -> Self {
        self.default_yes = Some(default_yes);
        self
    }

    pub fn build(self) -> Ui<W> {
        let no_color_env = std::env::var_os("NO_COLOR").is_some();
        let settings = current_settings();

        let config_color = self
            .color_enabled
            .or(self.config_overrides.color)
            .or(Some(settings.color_enabled));

        let color_enabled = if no_color_env {
            false
        } else {
            config_color.unwrap_or(true)
        };

        let default_yes = self
            .default_yes
            .or(self.config_overrides.default_yes)
            .or(Some(settings.default_yes))
            .unwrap_or(false);

        let spinner_style = theme::make_spinner_style(color_enabled);
        let progress_style = theme::make_progress_style(color_enabled);

        Ui {
            out: BufWriter::new(self.out),
            err: self.err,
            color_enabled,
            default_yes,
            spinner_style,
            progress_style,
        }
    }
}
