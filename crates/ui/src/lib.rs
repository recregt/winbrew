//! Terminal presentation primitives for WinBrew.
//!
//! `winbrew-ui` owns the interactive state used by CLI command handlers: color
//! mode, confirmation defaults, spinner styles, progress rendering, and output
//! writers. Keeping this crate separate from app logic prevents business code
//! from depending on terminal behavior.

#![allow(missing_docs)]

mod builder;
mod interact;
mod log;
mod progress;
mod table;
mod theme;

/// Builder for `Ui` instances.
pub use builder::UiBuilder;

use indicatif::ProgressStyle;
use std::io::{self, BufWriter, Write};
use std::sync::Arc;

/// Terminal-backed presentation state used by CLI command handlers.
pub struct Ui<W: Write> {
    pub(crate) out: BufWriter<W>,
    pub(crate) err: Box<dyn Write>,
    pub(crate) color_enabled: bool,
    pub(crate) default_yes: bool,
    pub(crate) spinner_style: Arc<ProgressStyle>,
    pub(crate) progress_style: Arc<ProgressStyle>,
}

impl Ui<io::Stdout> {
    /// Create a UI that writes to stdout and stderr.
    pub fn new(settings: UiSettings) -> Self {
        UiBuilder::new(settings).build()
    }
}

impl<W: Write> Ui<W> {
    /// Create a UI with an explicit writer, which is primarily useful in tests.
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
/// Presentation settings for the terminal UI.
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
