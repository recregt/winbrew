use super::{Ui, theme::SPINNER_TICK_MS};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::Write;
use std::time::Duration;

impl<W: Write> Ui<W> {
    pub fn progress_bar(&self) -> ProgressBar {
        let pb = ProgressBar::new(0);
        pb.set_style(ProgressStyle::clone(&self.progress_style));
        pb
    }

    pub fn spinner<T, F: FnOnce() -> T>(&self, message: impl Into<String>, f: F) -> T {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(ProgressStyle::clone(&self.spinner_style));
        spinner.set_message(message.into());
        spinner.enable_steady_tick(Duration::from_millis(SPINNER_TICK_MS));

        let result = f();
        spinner.finish_and_clear();
        result
    }
}
