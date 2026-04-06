#[path = "common/shared_root.rs"]
mod shared_root;

use anyhow::Result;
use md5::Md5;
use rusqlite::{Connection, params};
use sha2::{Digest, Sha512};
use shared_root::test_root;
use std::fs;
use std::io::{Read, Write};
use std::net::TcpListener;
use std::path::Path;
use std::thread;
use winbrew::AppContext;
use winbrew::database;
use winbrew::services::install;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

fn create_zip_archive(path: &Path, file_name: &str, contents: &[u8]) -> Result<()> {
    let file = fs::File::create(path)?;
    let mut writer = ZipWriter::new(file);
    writer.start_file(file_name, SimpleFileOptions::default())?;
    writer.write_all(contents)?;
    writer.finish()?;
    Ok(())
}

fn reset_install_state(root: &Path) -> Result<()> {
    let conn = database::get_conn()?;
    conn.execute("DELETE FROM installed_packages", [])?;

    let packages_dir = root.join("packages");
    if packages_dir.exists() {
        fs::remove_dir_all(&packages_dir)?;
    }
    fs::create_dir_all(&packages_dir)?;

    Ok(())
}

fn md5_hex(bytes: &[u8]) -> String {
    let mut hasher = Md5::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{:02x}", byte)).collect()
}

fn sha512_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha512::new();
    hasher.update(bytes);
    let digest = hasher.finalize();
    digest.iter().map(|byte| format!("{:02x}", byte)).collect()
}

fn init_context(root: &Path) -> Result<AppContext> {
    let config = database::Config::load_at(root)?;
    let context = AppContext::from_config(config)?;
    database::init(&context.paths)?;
    Ok(context)
}

fn start_file_server(
    body: Vec<u8>,
    content_type: &'static str,
) -> Result<(String, thread::JoinHandle<()>)> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let addr = listener.local_addr()?;

    let handle = thread::spawn(move || {
        if let Ok((mut stream, _peer)) = listener.accept() {
            let mut request_buffer = [0u8; 4096];
            let _ = stream.read(&mut request_buffer);

            let header = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nContent-Type: {}\r\nConnection: close\r\n\r\n",
                body.len(),
                content_type,
            );

            let _ = stream.write_all(header.as_bytes());
            let _ = stream.write_all(&body);
            let _ = stream.flush();
        }
    });

    Ok((format!("http://{addr}"), handle))
}

#[test]
fn install_runs_end_to_end_in_an_isolated_root() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();

    reset_install_state(root)?;

    let installer_dir = root.join("assets");
    fs::create_dir_all(&installer_dir)?;
    let zip_path = installer_dir.join("test.zip");
    create_zip_archive(&zip_path, "bin/tool.exe", b"zip-binary")?;
    let zip_bytes = fs::read(&zip_path)?;
    let sha512_hash = sha512_hex(&zip_bytes);

    let (base_url, server_handle) = start_file_server(zip_bytes, "application/zip")?;
    let installer_url = format!("{base_url}/test.zip");

    let catalog_db_dir = root.join("data").join("db");
    fs::create_dir_all(&catalog_db_dir)?;
    create_catalog_db_with_hash(
        &catalog_db_dir.join("catalog.db"),
        &installer_url,
        &sha512_hash,
    )?;

    let ctx = init_context(root)?;

    let result = install::run(
        &ctx,
        &["Winbrew Test Zip".to_string()],
        false,
        |_query, _matches| unreachable!("install should not prompt for an exact match"),
        |_| {},
        |_| {},
    )?;

    let install_dir = ctx.paths.packages.join("Winbrew Test Zip");
    assert_eq!(result.name, "Winbrew Test Zip");
    assert_eq!(result.version, "1.0.0");
    assert_eq!(result.install_dir, install_dir.to_string_lossy());
    assert!(install_dir.join("bin").join("tool.exe").exists());

    let conn = database::get_conn()?;
    let stored = database::get_package(&conn, "Winbrew Test Zip")?
        .expect("package should be marked as installed");
    assert_eq!(stored.status, winbrew::models::PackageStatus::Ok);
    assert_eq!(stored.kind, "zip");

    let _ = server_handle.join();

    Ok(())
}

#[test]
fn install_rejects_md5_without_override() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();

    reset_install_state(root)?;

    let installer_url = "https://example.invalid/test.zip".to_string();
    let md5_hash = "d41d8cd98f00b204e9800998ecf8427e".to_string();

    let catalog_db_dir = root.join("data").join("db");
    fs::create_dir_all(&catalog_db_dir)?;
    create_catalog_db_with_hash(
        &catalog_db_dir.join("catalog.db"),
        &installer_url,
        &md5_hash,
    )?;

    let ctx = init_context(root)?;

    let err = install::run(
        &ctx,
        &["Winbrew Test Zip".to_string()],
        false,
        |_query, _matches| unreachable!("install should not prompt for an exact match"),
        |_| {},
        |_| {},
    )
    .expect_err("md5 should be rejected without override");

    assert!(
        err.to_string()
            .contains("MD5 checksums are disabled by default")
    );

    Ok(())
}

#[test]
fn install_allows_md5_with_override() -> Result<()> {
    let test_root = test_root();
    let root = test_root.path();

    reset_install_state(root)?;

    let installer_dir = root.join("assets");
    fs::create_dir_all(&installer_dir)?;
    let zip_path = installer_dir.join("test.zip");
    create_zip_archive(&zip_path, "bin/tool.exe", b"zip-binary")?;
    let zip_bytes = fs::read(&zip_path)?;

    let (base_url, server_handle) = start_file_server(zip_bytes.clone(), "application/zip")?;
    let installer_url = format!("{base_url}/test.zip");
    let md5_hash = md5_hex(&zip_bytes);

    let catalog_db_dir = root.join("data").join("db");
    fs::create_dir_all(&catalog_db_dir)?;
    create_catalog_db_with_hash(
        &catalog_db_dir.join("catalog.db"),
        &installer_url,
        &md5_hash,
    )?;

    let ctx = init_context(root)?;

    let result = install::run(
        &ctx,
        &["Winbrew Test Zip".to_string()],
        true,
        |_query, _matches| unreachable!("install should not prompt for an exact match"),
        |_| {},
        |_| {},
    )?;

    let install_dir = ctx.paths.packages.join("Winbrew Test Zip");
    assert_eq!(result.name, "Winbrew Test Zip");
    assert!(install_dir.join("bin").join("tool.exe").exists());

    let _ = server_handle.join();

    Ok(())
}

fn create_catalog_db_with_hash(path: &Path, installer_url: &str, hash: &str) -> Result<()> {
    let conn = Connection::open(path)?;

    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS catalog_packages (
            id          TEXT PRIMARY KEY,
            name        TEXT NOT NULL,
            version     TEXT NOT NULL,
            description TEXT,
            homepage    TEXT,
            license     TEXT,
            publisher   TEXT
        );

        CREATE TABLE IF NOT EXISTS catalog_installers (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            package_id  TEXT NOT NULL REFERENCES catalog_packages(id) ON DELETE CASCADE,
            url         TEXT NOT NULL,
            hash        TEXT NOT NULL,
            arch        TEXT NOT NULL DEFAULT '',
            type        TEXT NOT NULL DEFAULT ''
        );

        CREATE VIRTUAL TABLE IF NOT EXISTS catalog_packages_fts USING fts5(
            name,
            description,
            content=catalog_packages,
            content_rowid=rowid
        );

        CREATE TRIGGER IF NOT EXISTS catalog_packages_ai AFTER INSERT ON catalog_packages BEGIN
            INSERT INTO catalog_packages_fts(rowid, name, description)
            VALUES (new.rowid, new.name, new.description);
        END;
        "#,
    )?;

    conn.execute("DELETE FROM catalog_installers", [])?;
    conn.execute("DELETE FROM catalog_packages", [])?;

    conn.execute(
        r#"
        INSERT INTO catalog_packages (
            id, name, version, description, homepage, license, publisher
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
        "#,
        params![
            "winget/Winbrew.TestZip",
            "Winbrew Test Zip",
            "1.0.0",
            Some("Synthetic package for isolated install testing"),
            Option::<String>::None,
            Option::<String>::None,
            Some("Winbrew Ltd."),
        ],
    )?;

    conn.execute(
        r#"
        INSERT INTO catalog_installers (
            package_id, url, hash, arch, type
        ) VALUES (?1, ?2, ?3, ?4, ?5)
        "#,
        params!["winget/Winbrew.TestZip", installer_url, hash, "", "zip",],
    )?;

    Ok(())
}
