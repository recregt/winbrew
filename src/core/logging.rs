use anyhow::{Context, Result};
use std::sync::OnceLock;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use crate::database::{Config, get_effective_value};

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();
static LOG_INIT: OnceLock<()> = OnceLock::new();

pub fn init() -> Result<()> {
    if LOG_INIT.get().is_some() {
        return Ok(());
    }

    let config = Config::current();
    let paths = config.resolved_paths();

    std::fs::create_dir_all(&paths.logs)
        .with_context(|| format!("failed to create log directory at {:?}", paths.logs))?;

    let file_appender = tracing_appender::rolling::daily(&paths.logs, "winbrew.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);

    let console_filter = get_effective_value("core.log_level")
        .map(|(value, _)| value)
        .unwrap_or_else(|_| config.core.log_level.clone());
    let file_filter = get_effective_value("core.file_log_level")
        .map(|(value, _)| value)
        .unwrap_or_else(|_| config.core.file_log_level.clone());

    let console_filter = EnvFilter::try_new(console_filter).context("invalid core.log_level")?;
    let file_filter = EnvFilter::try_new(file_filter).context("invalid core.file_log_level")?;

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
