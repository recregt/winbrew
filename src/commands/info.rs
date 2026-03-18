use anyhow::Result;

use crate::{core::paths, database, ui::Ui};

pub fn run() -> Result<()> {
    let ui = Ui::new();
    ui.page_title("Info");

    let conn = database::lock_conn()?;
    let install_root =
        paths::install_root(database::config_string(&conn, "install_dir")?.as_deref());

    let mut rows = Vec::new();
    rows.push(("Version".to_string(), version_string()));
    rows.push((
        "Database".to_string(),
        paths::db_path().to_string_lossy().to_string(),
    ));
    rows.push((
        "Install root".to_string(),
        install_root.to_string_lossy().to_string(),
    ));
    rows.push((
        "Packages dir".to_string(),
        paths::packages_dir_at(&install_root)
            .to_string_lossy()
            .to_string(),
    ));
    rows.push((
        "Bin dir".to_string(),
        paths::bin_dir_at(&install_root)
            .to_string_lossy()
            .to_string(),
    ));
    rows.push((
        "Cache dir".to_string(),
        paths::cache_dir_at(&install_root)
            .to_string_lossy()
            .to_string(),
    ));

    rows.push((
        "Registry URL".to_string(),
        database::config_string(&conn, "registry_url")?.unwrap_or_else(|| "(default)".to_string()),
    ));
    rows.push((
        "Proxy".to_string(),
        database::config_string(&conn, "proxy")?.unwrap_or_else(|| "(none)".to_string()),
    ));
    rows.push((
        "Download timeout".to_string(),
        database::config_u64(&conn, "download_timeout")?
            .map(|value| format!("{value}s"))
            .unwrap_or_else(|| "30s (default)".to_string()),
    ));
    rows.push((
        "Default yes".to_string(),
        database::config_bool(&conn, "default_yes")?
            .map(|value| value.to_string())
            .unwrap_or_else(|| "false (default)".to_string()),
    ));
    rows.push((
        "Color".to_string(),
        database::config_bool(&conn, "color")?
            .map(|value| value.to_string())
            .unwrap_or_else(|| "true (default)".to_string()),
    ));
    rows.push((
        "Concurrent downloads".to_string(),
        database::config_u64(&conn, "concurrent_downloads")?
            .map(|value| value.to_string())
            .unwrap_or_else(|| "(unset)".to_string()),
    ));
    rows.push((
        "GitHub token".to_string(),
        database::config_string(&conn, "github_token")?
            .map(|_| "(set)".to_string())
            .unwrap_or_else(|| "(unset)".to_string()),
    ));

    ui.display_key_values(&rows);
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
