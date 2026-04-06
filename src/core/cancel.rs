use anyhow::{Context, Result};
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

static HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);
static CANCELLED: AtomicBool = AtomicBool::new(false);

#[derive(Debug, Error)]
#[error("cancelled")]
pub struct CancellationError;

pub fn init_handler() -> Result<()> {
    if HANDLER_INSTALLED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(());
    }

    if let Err(err) = ctrlc::set_handler(|| {
        CANCELLED.store(true, Ordering::SeqCst);
    }) {
        HANDLER_INSTALLED.store(false, Ordering::SeqCst);
        return Err(err).context("failed to install Ctrl+C handler");
    }

    Ok(())
}

pub fn is_cancelled() -> bool {
    CANCELLED.load(Ordering::SeqCst)
}

pub fn check() -> std::result::Result<(), CancellationError> {
    if is_cancelled() {
        Err(CancellationError)
    } else {
        Ok(())
    }
}
