mod builder;
mod interact;
mod log;
mod progress;
mod table;
mod theme;

pub use builder::UiBuilder;

use std::sync::OnceLock;

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
    pub fn new() -> Self {
        UiBuilder::new().build()
    }
}

impl<W: Write> Ui<W> {
    pub fn with_writer(writer: W) -> Self {
        UiBuilder::with_writer(writer).build()
    }
}

impl Default for Ui<io::Stdout> {
    fn default() -> Self {
        Ui::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UiSettings {
    pub color_enabled: bool,
    pub default_yes: bool,
}

static UI_SETTINGS: OnceLock<UiSettings> = OnceLock::new();

pub fn init_settings(settings: UiSettings) {
    let _ = UI_SETTINGS.set(settings);
}

pub(crate) fn current_settings() -> UiSettings {
    *UI_SETTINGS.get_or_init(|| UiSettings {
        color_enabled: true,
        default_yes: false,
    })
}
