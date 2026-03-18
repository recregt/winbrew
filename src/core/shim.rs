use anyhow::{Context, Result, bail};
use std::fs;
use std::path::Path;

pub fn create(name: &str, target: &Path, args: Option<&str>) -> Result<()> {
    create_at(crate::core::paths::base_dir(), name, target, args)
}

pub fn create_at(root: &Path, name: &str, target: &Path, args: Option<&str>) -> Result<()> {
    let shim_path = crate::core::paths::shim_path_at(root, name);

    if let Some(parent) = shim_path.parent() {
        fs::create_dir_all(parent).context("failed to create bin directory")?;
    }

    let mut content = format!("path = {}\n", target.to_string_lossy());

    if let Some(args) = args.filter(|a| !a.is_empty()) {
        content.push_str("args = ");
        content.push_str(args);
        content.push('\n');
    }

    fs::write(&shim_path, content).context("failed to write shim file")?;

    Ok(())
}

pub fn remove(name: &str) -> Result<()> {
    remove_at(crate::core::paths::base_dir(), name)
}

pub fn remove_at(root: &Path, name: &str) -> Result<()> {
    let shim_path = crate::core::paths::shim_path_at(root, name);

    match fs::remove_file(&shim_path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(anyhow::Error::new(e).context("failed to remove shim file")),
    }
}

pub fn exists(name: &str) -> bool {
    exists_at(crate::core::paths::base_dir(), name)
}

pub fn exists_at(root: &Path, name: &str) -> bool {
    crate::core::paths::shim_path_at(root, name).exists()
}

pub fn read(name: &str) -> Result<(String, Option<String>)> {
    read_at(crate::core::paths::base_dir(), name)
}

pub fn read_at(root: &Path, name: &str) -> Result<(String, Option<String>)> {
    let shim_path = crate::core::paths::shim_path_at(root, name);

    let content = fs::read_to_string(&shim_path).context("failed to read shim file")?;

    let mut path = None;
    let mut args = None;

    for line in content.lines() {
        if let Some(val) = line.strip_prefix("path = ") {
            path = Some(val.to_owned());
        } else if let Some(val) = line.strip_prefix("args = ") {
            args = Some(val.to_owned());
        }
    }

    let path = match path {
        Some(p) if !p.is_empty() => p,
        _ => bail!("invalid or corrupt shim file: missing 'path' entry"),
    };

    Ok((path, args))
}
