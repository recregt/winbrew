use super::{Ui, theme::SPINNER_TICK_MS};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::Write;
use std::time::Duration;

#[must_use = "spinner guards must be held until the phase ends"]
pub struct SpinnerGuard(ProgressBar);

impl SpinnerGuard {
    pub fn update_message(&self, msg: impl Into<String>) {
        self.0.set_message(msg.into());
    }
}

impl Drop for SpinnerGuard {
    fn drop(&mut self) {
        self.0.finish_and_clear();
    }
}

impl<W: Write> Ui<W> {
    pub fn progress_bar(&self) -> ProgressBar {
        let pb = ProgressBar::new(0);
        pb.set_style(ProgressStyle::clone(&self.progress_style));
        pb
    }

    pub fn start_spinner(&self, message: impl Into<String>) -> SpinnerGuard {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(ProgressStyle::clone(&self.spinner_style));
        spinner.set_message(message.into());
        spinner.enable_steady_tick(Duration::from_millis(SPINNER_TICK_MS));
        SpinnerGuard(spinner)
    }

    pub fn spinner<T, F: FnOnce() -> T>(&self, message: impl Into<String>, f: F) -> T {
        let _guard = self.start_spinner(message);
        f()
    }
}
