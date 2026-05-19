use super::{Ui, theme::SPINNER_TICK_MS};
use indicatif::{ProgressBar, ProgressStyle};
use std::io::Write;
use std::time::Duration;

#[must_use = "progress handles must be held until the operation completes"]
pub struct ProgressHandle(Option<ProgressBar>);

impl ProgressHandle {
    fn progress(&self) -> &ProgressBar {
        self.0
            .as_ref()
            .expect("progress handle is unexpectedly empty")
    }

    fn clear(&mut self) {
        if let Some(progress) = self.0.take() {
            progress.finish_and_clear();
        }
    }

    pub fn set_length(&self, length: u64) {
        self.progress().set_length(length);
    }

    pub fn set_message(&self, message: impl Into<String>) {
        self.progress().set_message(message.into());
    }

    pub fn inc(&self, amount: u64) {
        self.progress().inc(amount);
    }

    pub fn finish_and_clear(mut self) {
        self.clear();
    }
}

impl Drop for ProgressHandle {
    fn drop(&mut self) {
        self.clear();
    }
}

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
    pub fn progress_bar(&self) -> ProgressHandle {
        let pb = ProgressBar::new(0);
        pb.set_style(ProgressStyle::clone(&self.progress_style));
        ProgressHandle(Some(pb))
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
