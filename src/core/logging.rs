use anyhow::{Context, Result};
use std::sync::OnceLock;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use crate::AppContext;

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();
static LOG_INIT: OnceLock<()> = OnceLock::new();

pub fn init(ctx: &AppContext) -> Result<()> {
    if LOG_INIT.get().is_some() {
        return Ok(());
    }

    std::fs::create_dir_all(&ctx.paths.logs)
        .with_context(|| format!("failed to create log directory at {:?}", ctx.paths.logs))?;

    let file_appender = tracing_appender::rolling::daily(&ctx.paths.logs, "winbrew.log");
    let (file_writer, guard) = tracing_appender::non_blocking(file_appender);
    let _ = LOG_GUARD.set(guard);

    let console_filter = EnvFilter::try_new(&ctx.log_level).context("invalid core.log_level")?;
    let file_filter =
        EnvFilter::try_new(&ctx.file_log_level).context("invalid core.file_log_level")?;

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
