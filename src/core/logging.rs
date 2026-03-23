use anyhow::{Context, Result};
use std::env;
use std::sync::OnceLock;
use tracing_subscriber::{EnvFilter, prelude::*};

use crate::database::Config;

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();

pub fn init() -> Result<()> {
    let config = Config::current();
    let paths = config.resolved_paths();

    std::fs::create_dir_all(&paths.logs).context("failed to create log directory")?;

    let file_appender = tracing_appender::rolling::daily(&paths.logs, "winbrew.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);

    let env_filter = match env::var_os("RUST_LOG").filter(|value| !value.is_empty()) {
        Some(value) => {
            let value = value.to_string_lossy();
            EnvFilter::try_new(value.as_ref()).context("invalid RUST_LOG value")?
        }
        None => EnvFilter::try_new(config.core.log_level.as_str())
            .unwrap_or_else(|_| EnvFilter::new("info")),
    };

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
