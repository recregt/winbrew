use anyhow::{Context, Result};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

static HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);
static CANCELLED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Error)]
#[error("cancelled")]
pub struct CancellationError;

pub fn init_handler() -> Result<()> {
    if HANDLER_INSTALLED
        .compare_exchange(false, true, Ordering::Relaxed, Ordering::Relaxed)
        .is_err()
    {
        return Ok(());
    }

    if let Err(err) = ctrlc::set_handler(|| {
        if CANCELLED.swap(true, Ordering::Relaxed) {
            process::exit(130);
        }
    }) {
        HANDLER_INSTALLED.store(false, Ordering::Relaxed);
        return Err(err).context("failed to install Ctrl+C handler");
    }

    Ok(())
}

pub fn is_cancelled() -> bool {
    CANCELLED.load(Ordering::Relaxed)
}

pub fn check() -> std::result::Result<(), CancellationError> {
    if is_cancelled() {
        Err(CancellationError)
    } else {
        Ok(())
    }
}
