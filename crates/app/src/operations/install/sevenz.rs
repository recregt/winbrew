use anyhow::{Context, Result};
use std::env;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use crate::core::env::WINBREW_PATHS_ROOT;
use crate::core::fs::{cleanup_path, replace_directory, system_sevenz_binary_path};
pub(crate) use crate::core::fs::{
    sevenz_bin_path_from_runtime_root, sevenz_dll_path_from_runtime_root,
    sevenz_runtime_dir_from_runtime_root,
};
use crate::core::hash::{hash_file, verify_hash};
use crate::core::network::{build_client, download_url_to_temp_file, is_7z_path};
use crate::models::shared::hash::HashAlgorithm;

use super::InstallError;

const SEVENZ_BOOTSTRAP_USER_AGENT: &str = "WinBrew 7-Zip bootstrap";
const SEVENZ_BOOTSTRAP_VERSION: &str = "26.00";
const SEVENZ_VERSION_FILENAME: &str = "VERSION";
const SEVENZR_FILENAME: &str = "7zr.exe";
const SEVENZR_DOWNLOAD_SHA256: &str =
    "sha256:4bec0bc59836a890a11568b58bd12a3e7b23a683557340562da211b6088058ba";
const SEVENZ_X86_DOWNLOAD_SHA256: &str =
    "sha256:d605eb609aa67796dca7cfe26d7e28792090bb8048302d6e05ede16e8e33145c";

fn sevenzr_download_url() -> String {
    format!(
        "https://github.com/ip7z/7zip/releases/download/{SEVENZ_BOOTSTRAP_VERSION}/{SEVENZR_FILENAME}"
    )
}

fn sevenz_x86_download_url() -> String {
    let x86_filename = sevenz_x86_filename();
    format!(
        "https://github.com/ip7z/7zip/releases/download/{SEVENZ_BOOTSTRAP_VERSION}/{x86_filename}"
    )
}

fn sevenz_x86_filename() -> String {
    format!("7z{}.exe", SEVENZ_BOOTSTRAP_VERSION.replace('.', ""))
}

pub(crate) fn sevenz_version_manifest_path(runtime_root: &Path) -> PathBuf {
    sevenz_runtime_dir_from_runtime_root(runtime_root).join(SEVENZ_VERSION_FILENAME)
}

pub(crate) fn runtime_root_env_guard(root: &Path) -> RuntimeRootEnvGuard {
    RuntimeRootEnvGuard::set(WINBREW_PATHS_ROOT, root)
}

pub(crate) fn ensure_runtime(
    runtime_root: &Path,
    installer_url: &str,
    mut confirm_runtime_bootstrap: impl FnMut(&str, &Path) -> Result<bool>,
) -> Result<(), InstallError> {
    if !runtime_bootstrap_required(runtime_root, installer_url) {
        return Ok(());
    }

    let runtime_dir = sevenz_runtime_dir_from_runtime_root(runtime_root);
    if !confirm_runtime_bootstrap("7-Zip runtime", &runtime_dir)? {
        return Err(InstallError::RuntimeBootstrapDeclined {
            runtime: "7-Zip runtime".to_string(),
        });
    }

    bootstrap_local_runtime(runtime_root).map_err(InstallError::from)
}

pub(crate) fn runtime_bootstrap_required(runtime_root: &Path, installer_url: &str) -> bool {
    is_7z_path(installer_url)
        && system_sevenz_binary_path().is_none()
        && !local_runtime_available(runtime_root)
}

fn local_runtime_available(runtime_root: &Path) -> bool {
    sevenz_bin_path_from_runtime_root(runtime_root).exists()
        && sevenz_dll_path_from_runtime_root(runtime_root).exists()
        && local_runtime_version_matches(runtime_root)
}

fn local_runtime_version_matches(runtime_root: &Path) -> bool {
    let version_path = sevenz_version_manifest_path(runtime_root);
    std::fs::read_to_string(&version_path)
        .map(|content| content.trim() == SEVENZ_BOOTSTRAP_VERSION)
        .unwrap_or(false)
}

fn bootstrap_local_runtime(runtime_root: &Path) -> Result<()> {
    let target_dir = sevenz_runtime_dir_from_runtime_root(runtime_root);
    let staging_dir = create_bootstrap_root();
    let sevenzr_path = staging_dir.join(SEVENZR_FILENAME);
    let sevenz_x86_filename = sevenz_x86_filename();
    let installer_path = staging_dir.join(&sevenz_x86_filename);
    let artifacts = BootstrapArtifacts::new(
        staging_dir.clone(),
        sevenzr_path.clone(),
        installer_path.clone(),
    );

    fs::create_dir_all(&staging_dir).with_context(|| {
        format!("failed to create 7z bootstrap staging directory {staging_dir:?}")
    })?;

    let client = build_client(SEVENZ_BOOTSTRAP_USER_AGENT)
        .context("failed to build 7z bootstrap HTTP client")?;

    let sevenzr_url = sevenzr_download_url();
    let sevenz_x86_url = sevenz_x86_download_url();

    download_verified_asset(
        &client,
        &sevenzr_url,
        &sevenzr_path,
        SEVENZR_FILENAME,
        SEVENZR_DOWNLOAD_SHA256,
    )?;
    download_verified_asset(
        &client,
        &sevenz_x86_url,
        &installer_path,
        &sevenz_x86_filename,
        SEVENZ_X86_DOWNLOAD_SHA256,
    )?;

    run_bootstrap_extractor(&sevenzr_path, &installer_path, &staging_dir)?;

    let version_path = staging_dir.join(SEVENZ_VERSION_FILENAME);
    fs::write(&version_path, SEVENZ_BOOTSTRAP_VERSION).with_context(|| {
        format!(
            "failed to write 7z bootstrap version file at {}",
            version_path.display()
        )
    })?;

    if let Some(parent) = target_dir.parent() {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create parent directory for {}",
                target_dir.display()
            )
        })?;
    }

    replace_directory(&staging_dir, &target_dir)
        .with_context(|| format!("failed to publish 7z runtime into {}", target_dir.display()))?;

    artifacts.commit();
    Ok(())
}

fn download_verified_asset(
    client: &crate::core::network::Client,
    url: &str,
    temp_path: &Path,
    label: &str,
    expected_hash: &str,
) -> Result<()> {
    download_url_to_temp_file(
        client,
        url,
        temp_path,
        label,
        |_| {},
        |_| {},
        |_| Ok::<(), crate::core::network::BoxError>(()),
    )
    .with_context(|| format!("failed to download {label}"))?;

    let actual_hash = hash_file(temp_path, HashAlgorithm::Sha256)
        .with_context(|| format!("failed to hash downloaded {label}"))?;

    verify_hash(expected_hash, actual_hash)
        .with_context(|| format!("downloaded {label} hash mismatch"))?;

    Ok(())
}

fn run_bootstrap_extractor(
    sevenzr_path: &Path,
    archive_path: &Path,
    destination_dir: &Path,
) -> Result<()> {
    let status = Command::new(sevenzr_path)
        .arg("x")
        .arg("-y")
        .arg("-bd")
        .arg(format!("-o{}", destination_dir.display()))
        .arg(archive_path)
        .arg("7z.exe")
        .arg("7z.dll")
        .status()
        .with_context(|| {
            format!(
                "failed to launch 7z bootstrap extractor at {}",
                sevenzr_path.display()
            )
        })?;

    if status.success() {
        Ok(())
    } else {
        anyhow::bail!("7zr exited with status {status}");
    }
}

fn create_bootstrap_root() -> PathBuf {
    let mut bootstrap_root = env::temp_dir();
    bootstrap_root.push(format!(
        "winbrew-7zip-bootstrap-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos()
    ));

    bootstrap_root
}

struct BootstrapArtifacts {
    staging_dir: PathBuf,
    sevenzr_path: PathBuf,
    installer_path: PathBuf,
    committed: bool,
}

impl BootstrapArtifacts {
    fn new(staging_dir: PathBuf, sevenzr_path: PathBuf, installer_path: PathBuf) -> Self {
        Self {
            staging_dir,
            sevenzr_path,
            installer_path,
            committed: false,
        }
    }

    fn commit(mut self) {
        let _ = cleanup_path(&self.staging_dir);
        let _ = fs::remove_file(&self.sevenzr_path);
        let _ = fs::remove_file(&self.installer_path);
        self.committed = true;
    }
}

impl Drop for BootstrapArtifacts {
    fn drop(&mut self) {
        if !self.committed {
            let _ = cleanup_path(&self.staging_dir);
            let _ = fs::remove_file(&self.sevenzr_path);
            let _ = fs::remove_file(&self.installer_path);
        }
    }
}

pub(crate) struct RuntimeRootEnvGuard {
    key: &'static str,
    previous: Option<OsString>,
}

impl RuntimeRootEnvGuard {
    fn set(key: &'static str, value: &Path) -> Self {
        let previous = env::var_os(key);
        unsafe {
            env::set_var(key, value);
        }

        Self { key, previous }
    }
}

impl Drop for RuntimeRootEnvGuard {
    fn drop(&mut self) {
        if let Some(previous) = self.previous.take() {
            unsafe {
                env::set_var(self.key, previous);
            }
        } else {
            unsafe {
                env::remove_var(self.key);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn bootstrap_urls_derive_from_version() {
        let expected_version = SEVENZ_BOOTSTRAP_VERSION;
        let expected_x86_filename = sevenz_x86_filename();
        assert_eq!(
            sevenzr_download_url(),
            format!(
                "https://github.com/ip7z/7zip/releases/download/{expected_version}/{SEVENZR_FILENAME}"
            )
        );
        assert_eq!(
            sevenz_x86_download_url(),
            format!(
                "https://github.com/ip7z/7zip/releases/download/{expected_version}/{expected_x86_filename}"
            )
        );
    }

    #[test]
    fn local_runtime_available_requires_matching_version_manifest() -> Result<()> {
        let temp_dir = tempdir().expect("temp dir");
        let runtime_root = temp_dir.path();
        let runtime_dir = sevenz_runtime_dir_from_runtime_root(runtime_root);
        fs::create_dir_all(&runtime_dir)?;

        fs::write(runtime_dir.join("7z.exe"), b"")?;
        fs::write(runtime_dir.join("7z.dll"), b"")?;
        fs::write(sevenz_version_manifest_path(runtime_root), b"25.50")?;

        assert!(!local_runtime_available(runtime_root));

        fs::write(
            sevenz_version_manifest_path(runtime_root),
            SEVENZ_BOOTSTRAP_VERSION,
        )?;
        assert!(local_runtime_available(runtime_root));

        Ok(())
    }

    #[test]
    fn runtime_bootstrap_required_rejects_mismatched_local_version() -> Result<()> {
        let temp_dir = tempdir().expect("temp dir");
        let runtime_root = temp_dir.path();
        let runtime_dir = sevenz_runtime_dir_from_runtime_root(runtime_root);

        fs::create_dir_all(&runtime_dir)?;
        fs::write(runtime_dir.join("7z.exe"), b"")?;
        fs::write(runtime_dir.join("7z.dll"), b"")?;
        fs::write(sevenz_version_manifest_path(runtime_root), b"25.50")?;

        assert!(runtime_bootstrap_required(
            runtime_root,
            "https://example.invalid/archive.7z"
        ));

        fs::write(
            sevenz_version_manifest_path(runtime_root),
            SEVENZ_BOOTSTRAP_VERSION,
        )?;
        assert!(!runtime_bootstrap_required(
            runtime_root,
            "https://example.invalid/archive.7z"
        ));

        Ok(())
    }
}
