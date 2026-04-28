use super::{Ui, theme::styled_line};
use console::Style;
use std::fmt::Display;
use std::io::Write;

impl<W: Write> Ui<W> {
    pub fn write_line(&mut self, message: impl Display) {
        let _ = writeln!(self.out, "{message}");
        let _ = self.out.flush();
    }

    pub fn info(&mut self, message: impl Display) {
        let _ = writeln!(self.err, "{message}");
    }

    /// `notice` is reserved for neutral status messages; may gain distinct
    /// formatting in future (e.g. dimmed). Prefer `info` for general output.
    pub fn notice(&mut self, message: impl Display) {
        let _ = writeln!(self.err, "{message}");
    }

    pub fn warn(&mut self, message: impl AsRef<str>) {
        let line = styled_line(
            self.color_enabled,
            "⚠",
            message.as_ref(),
            Style::new().yellow(),
        );
        let _ = writeln!(self.err, "{line}");
    }

    pub fn error(&mut self, message: impl AsRef<str>) {
        let line = styled_line(
            self.color_enabled,
            "✘",
            message.as_ref(),
            Style::new().red(),
        );
        let _ = writeln!(self.err, "{line}");
    }

    pub fn success(&mut self, message: impl AsRef<str>) {
        let line = styled_line(
            self.color_enabled,
            "✓",
            message.as_ref(),
            Style::new().green(),
        );
        let _ = writeln!(self.err, "{line}");
    }
}
