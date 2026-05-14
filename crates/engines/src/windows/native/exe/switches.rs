use anyhow::{Result, bail};
use std::path::Path;

use crate::models::catalog::package::CatalogInstaller;
use crate::models::install::installer::InstallerType;

pub(super) fn build_install_args(
    installer: &CatalogInstaller,
    install_dir: &Path,
    package_name: &str,
) -> Result<Vec<String>> {
    let mut args = installer
        .installer_switches
        .as_deref()
        .map(split_switches)
        .transpose()?
        .unwrap_or_default();

    match installer.kind {
        InstallerType::Exe => {
            if args.is_empty() {
                bail!(
                    "missing installer switches for generic exe installer '{}'",
                    package_name
                );
            }
        }
        InstallerType::Inno => {
            push_flag_if_missing(&mut args, "/VERYSILENT");
            push_flag_if_missing(&mut args, "/SUPPRESSMSGBOXES");
            push_flag_if_missing(&mut args, "/NORESTART");
            push_flag_if_missing(&mut args, "/SP-");

            if !has_arg_prefix(&args, "/dir=") {
                args.push(format!(r"/DIR={}", install_dir.display()));
            }
        }
        InstallerType::Nullsoft => {
            push_flag_if_missing(&mut args, "/S");

            if !has_arg_prefix(&args, "/d=") {
                args.push(format!(r"/D={}", install_dir.display()));
            }
        }
        InstallerType::Burn => {
            push_flag_if_missing(&mut args, "/quiet");
            push_flag_if_missing(&mut args, "/norestart");
        }
        _ => {
            bail!(
                "native exe backend cannot handle installer kind '{}'",
                installer.kind.as_str()
            )
        }
    }

    Ok(args)
}

pub(super) fn split_switches(raw: &str) -> Result<Vec<String>> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;

    for ch in raw.chars() {
        match ch {
            '"' | '\'' => match quote {
                Some(active) if active == ch => {
                    quote = None;
                }
                Some(_) => current.push(ch),
                None => quote = Some(ch),
            },
            ch if ch.is_whitespace() && quote.is_none() => {
                if !current.is_empty() {
                    args.push(std::mem::take(&mut current));
                }
            }
            ch => current.push(ch),
        }
    }

    if quote.is_some() {
        bail!("unterminated quoted installer switches: {raw}");
    }

    if !current.is_empty() {
        args.push(current);
    }

    validate_unique_switches(&args, raw)?;

    Ok(args)
}

fn validate_unique_switches(args: &[String], raw: &str) -> Result<()> {
    use std::collections::HashSet;

    let mut seen = HashSet::new();

    for arg in args {
        let signature = switch_signature(arg);

        if !seen.insert(signature) {
            bail!("duplicate installer switch detected: {arg} in {raw}");
        }
    }

    Ok(())
}

fn switch_signature(arg: &str) -> String {
    let trimmed = arg.trim();

    match trimmed.split_once('=') {
        Some((left, _)) => format!("{}=", left.to_ascii_lowercase()),
        None => trimmed.to_ascii_lowercase(),
    }
}

fn push_flag_if_missing(args: &mut Vec<String>, flag: &str) {
    if !args.iter().any(|arg| arg.eq_ignore_ascii_case(flag)) {
        args.push(flag.to_string());
    }
}

pub(super) fn has_arg_prefix(args: &[String], prefix: &str) -> bool {
    args.iter()
        .any(|arg| arg.to_ascii_lowercase().starts_with(prefix))
}
