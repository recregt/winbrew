//! Cancellation support for long-running CLI and workflow operations.
//!
//! The cancel helper installs a Ctrl+C handler once per process and exposes a
//! lightweight check that higher layers can call before expensive work.

use anyhow::{Context, Result};
use std::process;
use std::sync::atomic::{AtomicBool, Ordering};
use thiserror::Error;

static HANDLER_INSTALLED: AtomicBool = AtomicBool::new(false);
static CANCELLED: AtomicBool = AtomicBool::new(false);

/// Error returned when an operation is interrupted by cancellation.
#[derive(Debug, Error)]
#[error("cancelled")]
pub struct CancellationError;

/// Install the process-wide Ctrl+C handler once.
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

/// Return `true` when the process has observed cancellation.
pub fn is_cancelled() -> bool {
    CANCELLED.load(Ordering::Relaxed)
}

/// Return an error when the process has been cancelled.
pub fn check() -> std::result::Result<(), CancellationError> {
    if is_cancelled() {
        Err(CancellationError)
    } else {
        Ok(())
    }
}
