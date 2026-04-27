//! Command handler dispatch for WinBrew.
//!
//! This module maps parsed CLI commands to the concrete wrapper handlers in
//! the sibling modules. Each handler owns the user-facing UI behavior for its
//! command while delegating business logic to `winbrew-app`.

use anyhow::Result;

use crate::CommandContext;
use crate::cli::Command;

pub mod config;
pub mod doctor;
pub mod error;
pub mod info;
pub mod install;
pub mod list;
pub mod remove;
pub mod repair;
pub mod search;
pub mod update;
pub mod version;

/// Dispatch a parsed command to its wrapper handler.
pub fn run(
    command: Command,
    ctx: &CommandContext,
    config: &mut crate::database::Config,
) -> Result<()> {
    match command {
        Command::List { query } => list::run(ctx, &query),
        Command::Install {
            query,
            ignore_checksum_security,
            plan,
        } => install::run(ctx, &query, ignore_checksum_security, plan),
        Command::Search { query } => search::run(ctx, &query),
        Command::Info => info::run(ctx),
        Command::Version => version::run(ctx),
        Command::Doctor {
            json,
            warn_as_error,
        } => doctor::run(ctx, json, warn_as_error),
        Command::Update => update::run(ctx),
        Command::Remove { name, yes, force } => remove::run(ctx, &name, yes, force),
        Command::Repair { yes } => repair::run(ctx, yes),
        Command::Config { command } => config::run(ctx, config, command),
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::io::{Result as IoResult, Write};
    use std::sync::{Arc, Mutex};

    use winbrew_ui::{Ui, UiBuilder, UiSettings};

    pub(crate) type BufferBytes = Arc<Mutex<Vec<u8>>>;
    pub(crate) type BufferedUi = Ui<SharedBuffer>;
    pub(crate) type BufferedUiBundle = (BufferedUi, BufferBytes, BufferBytes);

    pub(crate) struct SharedBuffer {
        bytes: BufferBytes,
    }

    impl SharedBuffer {
        pub(crate) fn new(bytes: BufferBytes) -> Self {
            Self { bytes }
        }
    }

    impl Write for SharedBuffer {
        fn write(&mut self, buffer: &[u8]) -> IoResult<usize> {
            self.bytes
                .lock()
                .expect("buffer lock should be available")
                .extend_from_slice(buffer);
            Ok(buffer.len())
        }

        fn flush(&mut self) -> IoResult<()> {
            Ok(())
        }
    }

    pub(crate) fn buffered_ui(settings: UiSettings) -> BufferedUiBundle {
        let out: BufferBytes = Arc::new(Mutex::new(Vec::new()));
        let err: BufferBytes = Arc::new(Mutex::new(Vec::new()));
        let ui = UiBuilder::with_writer(SharedBuffer::new(Arc::clone(&out)), settings)
            .with_error_writer(Box::new(SharedBuffer::new(Arc::clone(&err))))
            .color_enabled(false)
            .build();

        (ui, out, err)
    }

    pub(crate) fn buffer_text(buffer: &BufferBytes) -> String {
        String::from_utf8(
            buffer
                .lock()
                .expect("buffer lock should be available")
                .clone(),
        )
        .expect("buffer should be utf-8")
    }
}
