use anyhow::{Context, Result};
use std::path::Path;
use std::sync::OnceLock;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();
static LOG_INIT: OnceLock<()> = OnceLock::new();

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
