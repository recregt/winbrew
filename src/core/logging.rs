use anyhow::{Context, Result};
use std::sync::OnceLock;
use tracing_subscriber::{EnvFilter, prelude::*};

use crate::core::paths;

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

pub fn init() -> Result<()> {
    paths::ensure_dirs().context("failed to create application directories")?;

    let file_appender = tracing_appender::rolling::daily(paths::log_dir(), "winbrew.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let console_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .without_time();
    let file_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .without_time()
        .with_ansi(false)
        .with_writer(file_writer);

    tracing_subscriber::registry()
        .with(env_filter)
        .with(console_layer)
        .with(file_layer)
        .init();

    Ok(())
}
