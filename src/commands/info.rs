use anyhow::Result;

use crate::{core::paths, database, ui::Ui};

pub fn run() -> Result<()> {
    let ui = Ui::new();
    ui.page_title("Info");

    let conn = database::lock_conn()?;
    let install_root =
        paths::install_root(database::config_string(&conn, "install_dir")?.as_deref());
    ui.notice(format!("Version: {}", version_string()));
    ui.notice(format!("Database: {}", paths::db_path().to_string_lossy()));
    ui.notice(format!("Install root: {}", install_root.to_string_lossy()));
    ui.notice(format!(
        "Packages dir: {}",
        paths::packages_dir_at(&install_root).to_string_lossy()
    ));
    ui.notice(format!(
        "Bin dir: {}",
        paths::bin_dir_at(&install_root).to_string_lossy()
    ));
    ui.notice(format!(
        "Cache dir: {}",
        paths::cache_dir_at(&install_root).to_string_lossy()
    ));
    ui.notice(format!(
        "Registry URL: {}",
        database::config_string(&conn, "registry_url")?.unwrap_or_else(|| "(default)".to_string())
    ));
    ui.notice(format!(
        "Proxy: {}",
        database::config_string(&conn, "proxy")?.unwrap_or_else(|| "(none)".to_string())
    ));
    ui.notice(format!(
        "Download timeout: {}",
        database::config_u64(&conn, "download_timeout")?
            .map(|value| format!("{value}s"))
            .unwrap_or_else(|| "30s (default)".to_string())
    ));
    ui.notice(format!(
        "Default yes: {}",
        database::config_bool(&conn, "default_yes")?
            .map(|value| value.to_string())
            .unwrap_or_else(|| "false (default)".to_string())
    ));
    ui.notice(format!(
        "Color: {}",
        database::config_bool(&conn, "color")?
            .map(|value| value.to_string())
            .unwrap_or_else(|| "true (default)".to_string())
    ));
    ui.notice(format!(
        "Concurrent downloads: {}",
        database::config_u64(&conn, "concurrent_downloads")?
            .map(|value| value.to_string())
            .unwrap_or_else(|| "(unset)".to_string())
    ));
    ui.notice(format!(
        "GitHub token: {}",
        database::config_string(&conn, "github_token")?
            .map(|_| "(set)".to_string())
            .unwrap_or_else(|| "(unset)".to_string())
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
