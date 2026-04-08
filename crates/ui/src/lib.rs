#![allow(missing_docs)]

mod builder;
mod interact;
mod log;
mod progress;
mod table;
mod theme;

pub use builder::UiBuilder;

use indicatif::ProgressStyle;
use std::io::{self, BufWriter, Write};
use std::sync::Arc;

pub struct Ui<W: Write> {
    pub(crate) out: BufWriter<W>,
    pub(crate) err: Box<dyn Write>,
    pub(crate) color_enabled: bool,
    pub(crate) default_yes: bool,
    pub(crate) spinner_style: Arc<ProgressStyle>,
    pub(crate) progress_style: Arc<ProgressStyle>,
}

impl Ui<io::Stdout> {
    pub fn new(settings: UiSettings) -> Self {
        UiBuilder::new(settings).build()
    }
}

impl<W: Write> Ui<W> {
    pub fn with_writer(writer: W, settings: UiSettings) -> Self {
        UiBuilder::with_writer(writer, settings).build()
    }
}

impl Default for Ui<io::Stdout> {
    fn default() -> Self {
        let settings = UiSettings::default();
        let spinner_style = crate::theme::make_spinner_style(settings.color_enabled);
        let progress_style = crate::theme::make_progress_style(settings.color_enabled);

        Ui {
            out: BufWriter::new(io::stdout()),
            err: Box::new(io::stderr()),
            color_enabled: settings.color_enabled,
            default_yes: settings.default_yes,
            spinner_style,
            progress_style,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiSettings {
    pub color_enabled: bool,
    pub default_yes: bool,
}

impl Default for UiSettings {
    fn default() -> Self {
        Self {
            color_enabled: true,
            default_yes: false,
        }
    }
}
