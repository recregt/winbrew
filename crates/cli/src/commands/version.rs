//! Version command wrapper for the binary version string.

use anyhow::Result;
use std::io::{self, Write};

use crate::{CommandContext, app::version};

pub fn run(_ctx: &CommandContext) -> Result<()> {
    let mut stdout = io::stdout();
    emit_version(&mut stdout)
}

fn emit_version<W: Write>(writer: &mut W) -> Result<()> {
    writeln!(writer, "{}", version::version_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::emit_version;
    use crate::app::version::version_string;
    use crate::commands::test_support::{SharedBuffer, buffer_text};
    use std::sync::{Arc, Mutex};

    #[test]
    fn emit_version_writes_version_string() {
        let output = Arc::new(Mutex::new(Vec::new()));
        let mut writer = SharedBuffer::new(Arc::clone(&output));

        emit_version(&mut writer).expect("version should be written");

        assert_eq!(buffer_text(&output), format!("{}\n", version_string()));
    }
}
