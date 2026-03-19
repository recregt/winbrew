use comfy_table::{Cell, Color};
use console::Style;
use indicatif::ProgressStyle;
use std::sync::Arc;

pub const DEFAULT_TABLE_WIDTH: u16 = 80;

pub fn terminal_width() -> u16 {
    term_size::dimensions()
        .map(|(w, _)| w as u16)
        .unwrap_or(DEFAULT_TABLE_WIDTH)
}

pub const SPINNER_TICK_MS: u64 = 80;

const SPINNER_TEMPLATE_COLOR: &str = "{spinner:.green} {msg}";
const SPINNER_TEMPLATE_PLAIN: &str = "{spinner} {msg}";

const PROGRESS_TEMPLATE_COLOR: &str =
    "{spinner:.green} [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})";
const PROGRESS_TEMPLATE_PLAIN: &str = "{spinner} [{bar:40}] {bytes}/{total_bytes} ({eta})";

const PROGRESS_CHARS: &str = "█▉▊▋▌▍▎▏  ";

pub fn header_cell(label: &str, color_enabled: bool, fg: Color) -> Cell {
    let cell = Cell::new(label);
    if color_enabled { cell.fg(fg) } else { cell }
}

pub fn make_spinner_style(color_enabled: bool) -> Arc<ProgressStyle> {
    let template = if color_enabled {
        SPINNER_TEMPLATE_COLOR
    } else {
        SPINNER_TEMPLATE_PLAIN
    };

    let ticks: &[&str] = if color_enabled {
        &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏", ""]
    } else {
        &["-", "\\", "|", "/", "-"]
    };

    Arc::new(
        ProgressStyle::with_template(template)
            .expect("spinner template must be valid")
            .tick_strings(ticks),
    )
}

pub fn make_progress_style(color_enabled: bool) -> Arc<ProgressStyle> {
    let template = if color_enabled {
        PROGRESS_TEMPLATE_COLOR
    } else {
        PROGRESS_TEMPLATE_PLAIN
    };

    Arc::new(
        ProgressStyle::with_template(template)
            .expect("progress template must be valid")
            .progress_chars(PROGRESS_CHARS),
    )
}

pub fn styled_line(value: bool, icon: &str, msg: &str, style: Style) -> String {
    if value {
        let level = style.clone().bold().apply_to(icon);
        let body = style.apply_to(msg);
        format!("{level} {body}")
    } else {
        format!("{icon} {msg}")
    }
}
