use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

use crate::core::{fs::cleanup_path, network::installer_filename, temp_workspace};
use crate::engines;
use crate::operations::install;

use super::ResolvedFileRestoreTarget;

/// Restore the drifting files from a staged package tree.
pub fn restore_file_restore_target(
    target: &ResolvedFileRestoreTarget,
    target_paths: &[PathBuf],
) -> Result<usize> {
    let temp_root =
        temp_workspace::build_temp_root(&target.package.name, &target.package.version.to_string());
    cleanup_path(&temp_root)?;
    fs::create_dir_all(&temp_root)?;

    let result = (|| -> Result<usize> {
        let stage_dir = temp_root.join("stage");
        let client = install::download::build_client()?;
        let download_path = temp_root.join(installer_filename(&target.installer.url));

        install::download::download_installer(
            &client,
            &target.installer,
            &download_path,
            false,
            |_| {},
            |_| {},
        )?;

        let resolved_kind =
            engines::resolve_downloaded_installer_kind(&target.installer, &download_path)?;
        let mut resolved_installer = target.installer.clone();
        resolved_installer.kind = resolved_kind;
        let engine = engines::resolve_engine_for_installer(&resolved_installer)?;

        let _ = install::flow::execute_engine_install(
            engine,
            &resolved_installer,
            &download_path,
            &stage_dir,
            &target.package.name,
        )?;

        restore_target_files(
            &stage_dir,
            Path::new(&target.installed_package.install_dir),
            target_paths,
        )
    })();

    let _ = cleanup_path(&temp_root);

    result
}

pub(crate) fn restore_target_files(
    stage_dir: &Path,
    install_dir: &Path,
    target_paths: &[PathBuf],
) -> Result<usize> {
    let mut restored = 0usize;

    for target_path in target_paths {
        let relative_path = target_path.strip_prefix(install_dir).with_context(|| {
            format!(
                "failed to derive restored file path for {} from {}",
                target_path.display(),
                install_dir.display()
            )
        })?;
        let source_path = stage_dir.join(relative_path);

        if let Some(parent) = target_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "failed to prepare parent directory for {}",
                    target_path.display()
                )
            })?;
        }

        fs::copy(&source_path, target_path).with_context(|| {
            format!(
                "failed to restore file {} from staged package",
                target_path.display()
            )
        })?;

        restored += 1;
    }

    Ok(restored)
}

#[cfg(test)]
mod tests {
    use super::restore_target_files;
    use anyhow::Result;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn restore_target_files_copies_staged_content() -> Result<()> {
        let root = tempdir().expect("temp dir");
        let stage_dir = root.path().join("stage");
        let install_dir = root.path().join("packages").join("Contoso.App");
        let target_path = install_dir.join("bin").join("tool.exe");
        let staged_path = stage_dir.join("bin").join("tool.exe");

        fs::create_dir_all(staged_path.parent().expect("stage parent")).expect("stage dir");
        fs::create_dir_all(target_path.parent().expect("target parent")).expect("target dir");
        fs::write(&staged_path, b"restored-binary").expect("write staged file");

        let restored =
            restore_target_files(&stage_dir, &install_dir, std::slice::from_ref(&target_path))?;

        assert_eq!(restored, 1);
        assert_eq!(
            fs::read(&target_path).expect("read target"),
            b"restored-binary"
        );

        Ok(())
    }
}
