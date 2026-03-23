use anyhow::Result;

use crate::{database::Config, ui::Ui};

pub fn run() -> Result<()> {
    let mut ui = Ui::new();
    ui.page_title("Info");

    let config = Config::current();
    let paths = config.resolved_paths();
    ui.notice(format!("Version: {}", version_string()));
    ui.notice(format!("Database: {}", paths.db.to_string_lossy()));
    ui.notice(format!("Config file: {}", paths.config.to_string_lossy()));
    ui.notice(format!("Log file: {}", paths.log.to_string_lossy()));
    ui.notice(format!("Install root: {}", paths.root.to_string_lossy()));
    ui.notice(format!(
        "Packages dir: {}",
        paths.packages.to_string_lossy()
    ));
    ui.notice(format!("Bin dir: {}", paths.bin.to_string_lossy()));
    ui.notice(format!("Cache dir: {}", paths.cache.to_string_lossy()));
    ui.notice(format!(
        "Registry URL: {}",
        config.sources.winget.url.as_str()
    ));
    ui.notice(format!(
        "Proxy: {}",
        config.core.proxy.as_deref().unwrap_or("(none)")
    ));
    ui.notice(format!(
        "Download timeout: {}s",
        config.core.download_timeout
    ));
    ui.notice(format!("Default yes: {}", config.core.default_yes));
    ui.notice(format!("Color: {}", config.core.color));
    ui.notice(format!(
        "Concurrent downloads: {}",
        config.core.concurrent_downloads
    ));
    ui.notice(format!(
        "GitHub token: {}",
        config
            .core
            .github_token
            .as_deref()
            .map(|_| "(set)")
            .unwrap_or("(unset)")
    ));

    ui.success("Runtime settings displayed.");

    Ok(())
}

fn version_string() -> String {
    format!(
        "{} ({})",
        env!("CARGO_PKG_VERSION"),
        env!("WINBREW_GIT_HASH")
    )
}
