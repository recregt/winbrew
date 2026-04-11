use std::path::{Path, PathBuf};

use crate::hash::Hasher;
use winbrew_models::HashAlgorithm;

#[derive(Debug, Clone)]
pub struct ResolvedPaths {
    pub root: PathBuf,
    pub packages: PathBuf,
    pub data: PathBuf,
    pub logs: PathBuf,
    pub cache: PathBuf,
    pub db: PathBuf,
    pub catalog_db: PathBuf,
    pub config: PathBuf,
    pub log: PathBuf,
}

pub fn config_file_at(root: &Path) -> PathBuf {
    root.join("data").join("winbrew.toml")
}

pub fn packages_dir_at(root: &Path) -> PathBuf {
    root.join("packages")
}

pub fn data_dir_at(root: &Path) -> PathBuf {
    root.join("data")
}

pub fn pkgdb_dir_at(root: &Path) -> PathBuf {
    data_dir_at(root).join("pkgdb")
}

pub fn db_dir_at(root: &Path) -> PathBuf {
    data_dir_at(root).join("db")
}

pub fn db_path_at(root: &Path) -> PathBuf {
    db_dir_at(root).join("winbrew.db")
}

pub fn catalog_db_at(root: &Path) -> PathBuf {
    db_dir_at(root).join("catalog.db")
}

pub fn log_dir_at(root: &Path) -> PathBuf {
    root.join("data").join("logs")
}

pub fn log_file_at(root: &Path) -> PathBuf {
    log_dir_at(root).join("winbrew.log")
}

pub fn cache_dir_at(root: &Path) -> PathBuf {
    root.join("data").join("cache")
}

pub fn cache_file_at(root: &Path, name: &str, version: &str, ext: &str) -> PathBuf {
    cache_dir_at(root).join(cache_filename(name, version, ext))
}

pub fn package_journal_key(package_id: &str, version: &str) -> String {
    let mut key = sanitize_package_key_component(package_id);
    key.push('-');
    key.push_str(&version_hash(version));
    key
}

pub fn package_journal_dir_at(root: &Path, package_key: &str) -> PathBuf {
    pkgdb_dir_at(root).join(package_key)
}

pub fn package_journal_file_at(root: &Path, package_key: &str) -> PathBuf {
    package_journal_dir_at(root, package_key).join("journal.jsonl")
}

fn cache_filename(name: &str, version: &str, ext: &str) -> String {
    let mut filename = String::with_capacity(name.len() + version.len() + ext.len() + 2);
    filename.push_str(name);
    filename.push('-');
    filename.push_str(version);
    filename.push('.');
    filename.push_str(ext);
    filename
}

pub fn ensure_dirs_at(root: &Path) -> std::io::Result<()> {
    for dir in [
        packages_dir_at(root),
        data_dir_at(root),
        pkgdb_dir_at(root),
        db_dir_at(root),
        log_dir_at(root),
        cache_dir_at(root),
    ] {
        std::fs::create_dir_all(dir)?;
    }

    Ok(())
}

pub fn ensure_install_dirs_at(root: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(packages_dir_at(root))
}

pub fn resolve_template(root: &Path, template: &str) -> PathBuf {
    let root_text = root.to_string_lossy();

    if template.contains("${root}") {
        PathBuf::from(template.replace("${root}", &root_text))
    } else {
        PathBuf::from(template)
    }
}

pub fn resolved_paths(
    root: &Path,
    packages: &str,
    data: &str,
    logs: &str,
    cache: &str,
) -> ResolvedPaths {
    let root = PathBuf::from(root);
    let data = resolve_template(&root, data);
    let logs = resolve_template(&root, logs);
    let db = data.join("db");

    ResolvedPaths {
        packages: resolve_template(&root, packages),
        cache: resolve_template(&root, cache),
        db: db.join("winbrew.db"),
        catalog_db: db.join("catalog.db"),
        config: data.join("winbrew.toml"),
        log: logs.join("winbrew.log"),
        root,
        data,
        logs,
    }
}

pub fn install_root_from_package_dir(install_dir: &Path) -> PathBuf {
    install_dir
        .parent()
        .and_then(|path| path.parent())
        .map(PathBuf::from)
        .unwrap_or_default()
}

fn sanitize_package_key_component(value: &str) -> String {
    let mut normalized = String::with_capacity(value.len());

    for ch in value.trim().chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            normalized.push(ch);
        } else {
            normalized.push('_');
        }
    }

    if normalized.is_empty() {
        "package".to_string()
    } else {
        normalized
    }
}

fn version_hash(version: &str) -> String {
    let mut hasher = Hasher::new(HashAlgorithm::Sha256);
    hasher.update(version.trim().as_bytes());

    let digest = hasher.finalize();
    let mut encoded = String::with_capacity(16);
    const HEX: &[u8; 16] = b"0123456789abcdef";

    for &byte in digest.iter().take(8) {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0F) as usize] as char);
    }

    encoded
}

#[cfg(test)]
mod tests {
    use super::{ensure_dirs_at, package_journal_file_at, package_journal_key, pkgdb_dir_at};
    use sha2::{Digest, Sha256};
    use tempfile::tempdir;

    #[test]
    fn package_journal_key_includes_sanitized_id_and_version_hash() {
        let version = "1.2.3";
        let expected_hash = {
            let digest = Sha256::digest(version.trim().as_bytes());
            let mut encoded = String::with_capacity(16);
            const HEX: &[u8; 16] = b"0123456789abcdef";

            for &byte in digest.iter().take(8) {
                encoded.push(HEX[(byte >> 4) as usize] as char);
                encoded.push(HEX[(byte & 0x0F) as usize] as char);
            }

            encoded
        };

        let key = package_journal_key("winget/Contoso.App", version);

        assert_eq!(key, format!("winget_Contoso.App-{expected_hash}"));
    }

    #[test]
    fn package_journal_file_lives_under_pkgdb() {
        let root = tempdir().expect("temp dir");
        let package_key = package_journal_key("winget/Contoso.App", "1.0.0");

        let journal_file = package_journal_file_at(root.path(), &package_key);

        assert_eq!(
            journal_file,
            pkgdb_dir_at(root.path())
                .join(&package_key)
                .join("journal.jsonl")
        );
    }

    #[test]
    fn ensure_dirs_creates_pkgdb_directory() {
        let root = tempdir().expect("temp dir");

        ensure_dirs_at(root.path()).expect("create dirs");

        assert!(pkgdb_dir_at(root.path()).exists());
    }
}
