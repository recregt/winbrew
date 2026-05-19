use anyhow::Result;
use clap::CommandFactory;
use clap_complete::{
    generate,
    shells::{Bash, Fish, PowerShell, Zsh},
};
use std::io::{self, Write};

use crate::cli::{Cli, CompletionShell};

pub fn run(shell: CompletionShell) -> Result<()> {
    let mut stdout = io::stdout().lock();

    write_completion(shell, &mut stdout);

    stdout.flush()?;
    Ok(())
}

pub(crate) fn write_completion<W: Write>(shell: CompletionShell, writer: &mut W) {
    let mut command = Cli::command();
    let bin_name = command.get_name().to_string();

    match shell {
        CompletionShell::Bash => generate(Bash, &mut command, bin_name, writer),
        CompletionShell::Fish => generate(Fish, &mut command, bin_name, writer),
        CompletionShell::Zsh => generate(Zsh, &mut command, bin_name, writer),
        CompletionShell::PowerShell => generate(PowerShell, &mut command, bin_name, writer),
    }
}

#[cfg(test)]
mod tests {
    use super::{CompletionShell, write_completion};

    #[test]
    fn bash_completion_generation_writes_output() {
        let mut buffer = Vec::new();

        write_completion(CompletionShell::Bash, &mut buffer);

        assert!(!buffer.is_empty());
    }
}
