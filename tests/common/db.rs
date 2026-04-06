use anyhow::Result;
use std::fs;
use std::path::Path;
use winbrew::database;

pub fn init_database(root: &Path) -> Result<()> {
    let config = database::Config::load_at(root)?;
    database::init(&config.resolved_paths())
}

pub fn reset_install_state(root: &Path) -> Result<()> {
    let conn = database::get_conn()?;
    conn.execute("DELETE FROM installed_packages", [])?;

    let packages_dir = root.join("packages");
    if packages_dir.exists() {
        fs::remove_dir_all(&packages_dir)?;
    }
    fs::create_dir_all(&packages_dir)?;

    Ok(())
}

pub fn reset_installed_packages(conn: &rusqlite::Connection) -> Result<()> {
    conn.execute("DELETE FROM installed_packages", [])?;

    Ok(())
}
