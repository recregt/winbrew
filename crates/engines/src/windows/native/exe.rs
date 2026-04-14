use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;
use std::process::Command;

use winbrew_core::fs::cleanup_path;
use winbrew_models::catalog::package::CatalogInstaller;
use winbrew_models::install::engine::{EngineInstallReceipt, EngineKind};
use winbrew_models::install::installed::InstalledPackage;
use winbrew_models::install::installer::InstallerType;

const NATIVE_EXE_SUCCESS_EXIT_CODES: &[i32] = &[0, 1641, 3010];

pub fn install(
    installer: &CatalogInstaller,
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<EngineInstallReceipt> {
    #[cfg(not(windows))]
    {
        let _ = (installer, download_path, install_dir, package_name);
        bail!("native executable installation is only supported on Windows")
    }

    #[cfg(windows)]
    {
        fs::create_dir_all(install_dir)
            .with_context(|| format!("failed to create {}", install_dir.display()))?;

        let args = build_install_args(installer, install_dir, package_name)?;

        let status = Command::new(download_path)
            .current_dir(download_path.parent().unwrap_or(Path::new(".")))
            .args(&args)
            .status()
            .with_context(|| {
                format!("failed to launch native executable installer for {package_name}")
            })?;

        let exit_code = status.code().ok_or_else(|| {
            anyhow::anyhow!("native executable installer terminated without an exit code")
        })?;

        if !NATIVE_EXE_SUCCESS_EXIT_CODES.contains(&exit_code) {
            bail!(
                "native executable installer for {} failed with exit code {}",
                package_name,
                exit_code
            );
        }

        Ok(EngineInstallReceipt::new(
            EngineKind::NativeExe,
            install_dir.to_string_lossy().into_owned(),
            None,
        ))
    }
}

pub fn remove(package: &InstalledPackage) -> Result<()> {
    #[cfg(not(windows))]
    {
        let _ = package;
        bail!("native executable removal is only supported on Windows")
    }

    #[cfg(windows)]
    {
        cleanup_path(Path::new(&package.install_dir))
            .with_context(|| format!("failed to remove {}", package.install_dir))?;

        Ok(())
    }
}

#[cfg(windows)]
fn build_install_args(
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

fn split_switches(raw: &str) -> Result<Vec<String>> {
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

    Ok(args)
}

fn push_flag_if_missing(args: &mut Vec<String>, flag: &str) {
    if !args.iter().any(|arg| arg.eq_ignore_ascii_case(flag)) {
        args.push(flag.to_string());
    }
}

fn has_arg_prefix(args: &[String], prefix: &str) -> bool {
    args.iter()
        .any(|arg| arg.to_ascii_lowercase().starts_with(prefix))
}

#[cfg(test)]
mod tests {
    use super::{has_arg_prefix, split_switches};

    #[test]
    fn split_switches_preserves_quoted_arguments() {
        let args = split_switches(r#"/S /D="C:\Program Files\Demo" /quiet"#)
            .expect("switches should parse");

        assert_eq!(
            args,
            vec![
                "/S".to_string(),
                "/D=C:\\Program Files\\Demo".to_string(),
                "/quiet".to_string(),
            ]
        );
    }

    #[test]
    fn has_arg_prefix_detects_case_insensitive_prefixes() {
        let args = vec!["/DIR=C:\\Tools\\App".to_string()];

        assert!(has_arg_prefix(&args, "/dir="));
    }
}
