//! One-time tracing and log-sink initialization for the CLI process.
//!
//! The CLI initializes logging before database startup and command dispatch so
//! any later failure, including bootstrap cleanup failures, can be written to
//! both the terminal and the rotating log file. This module intentionally keeps
//! the global tracing subscriber setup isolated from the rest of startup.

use anyhow::{Context, Result};
use std::path::Path;
use std::sync::OnceLock;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();
static LOG_INIT: OnceLock<()> = OnceLock::new();

/// Initialize the process-wide tracing subscriber and log file sink.
/// The function is idempotent: the first successful call installs the global
/// subscriber, creates the log directory if needed, and keeps the file writer
/// guard alive for the remainder of the process. Subsequent calls are no-ops.
/// `log_level` controls the console filter, while `file_log_level` controls the
/// file sink. Both are parsed through `EnvFilter`, so the configuration accepts
/// standard tracing filter syntax rather than a bespoke log-level enum.
pub fn init(log_dir: &Path, log_level: &str, file_log_level: &str) -> Result<()> {
    if LOG_INIT.get().is_some() {
        return Ok(());
    }

    std::fs::create_dir_all(log_dir)
        .with_context(|| format!("failed to create log directory at {:?}", log_dir))?;

    let file_appender = tracing_appender::rolling::daily(log_dir, "winbrew.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);

    let console_filter = EnvFilter::try_new(log_level).context("invalid core.log_level")?;
    let file_filter = EnvFilter::try_new(file_log_level).context("invalid core.file_log_level")?;

    let console_layer = fmt::layer()
        .with_target(false)
        .without_time()
        .with_filter(console_filter);

    let file_layer = fmt::layer()
        .with_target(true)
        .with_thread_ids(true)
        .with_ansi(false)
        .with_writer(file_writer)
        .with_filter(file_filter);

    tracing_subscriber::registry()
        .with(console_layer)
        .with(file_layer)
        .try_init()
        .context("failed to initialize tracing subscriber")?;

    let _ = LOG_INIT.set(());

    Ok(())
}
