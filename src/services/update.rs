use anyhow::{Context, Result, bail};
use std::path::PathBuf;

pub fn refresh_catalog() -> Result<()> {
    let crawler_dir = crawler_working_dir()?;

    let status = std::process::Command::new("go")
        .current_dir(&crawler_dir)
        .args(["run", "./cmd/crawler"])
        .status()
        .context("failed to start the Go catalog crawler")?;

    if !status.success() {
        bail!("Go catalog crawler failed with code: {:?}", status.code());
    }

    Ok(())
}

fn crawler_working_dir() -> Result<PathBuf> {
    let current_dir = std::env::current_dir().context("failed to resolve current directory")?;

    if current_dir.join("config.yaml").exists() && current_dir.join("cmd/crawler").exists() {
        return Ok(current_dir);
    }

    let infra_dir = current_dir.join("infra");
    if infra_dir.join("config.yaml").exists() && infra_dir.join("cmd/crawler").exists() {
        return Ok(infra_dir);
    }

    Ok(current_dir)
}