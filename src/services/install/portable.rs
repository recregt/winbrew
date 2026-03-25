use anyhow::{Context, Result};

use crate::core::install::{InstallTransaction, source_file_name};
use crate::core::network::NetworkSettings;
use crate::core::network::download_and_verify;

use super::InstallPlan;

pub fn install(
    conn: &rusqlite::Connection,
    context: &InstallPlan,
    on_progress: &mut impl FnMut(u64, u64),
) -> Result<()> {
    let tx = InstallTransaction::start(conn, context)?;
    let settings = NetworkSettings::current();

    let install_file_name = install_file_name(context)?;
    let install_file = context.install_dir.join(install_file_name);

    download_and_verify(
        &settings,
        &context.source.url,
        &context.cache_file,
        &context.source.checksum,
        on_progress,
    )
    .context("download and verification failed")?;

    std::fs::copy(&context.cache_file, &install_file).context("failed to copy portable package")?;

    tx.commit()
}

fn install_file_name(context: &InstallPlan) -> Result<String> {
    source_file_name(&context.source.url).context("could not determine file name from source URL")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::install::{detect_ext, install_root};
    use crate::manifest::{InstallerEntry, Manifest, ManifestInfo, Package, Source};

    fn build_context(url: &str) -> InstallPlan {
        let source = Source {
            url: url.to_string(),
            checksum: "abc123".to_string(),
            kind: "portable".to_string(),
        };

        let manifest = Manifest {
            manifest: ManifestInfo::default(),
            package: Package {
                name: "Microsoft.WindowsTerminal".to_string(),
                version: "1.21.2361.0".to_string(),
                package_name: Some("Windows Terminal".to_string()),
                description: Some("Terminal".to_string()),
                publisher: Some("Microsoft Corporation".to_string()),
                homepage: None,
                license: None,
                moniker: None,
                tags: vec![],
                dependencies: vec!["Microsoft.VCLibs".to_string()],
            },
            source: Some(source.clone()),
            installers: vec![InstallerEntry {
                architecture: "x64".to_string(),
                installer_type: "portable".to_string(),
                installer_url: url.to_string(),
                installer_sha256: "abc123".to_string(),
                installer_locale: None,
                scope: None,
                product_code: None,
                release_date: None,
                display_name: None,
                upgrade_behavior: None,
            }],
            metadata: None,
        };

        let install_root = install_root();
        let install_dir = crate::core::paths::package_dir_at(&install_root, &manifest.package.name);

        InstallPlan {
            name: manifest.package.name.clone(),
            package_version: manifest.package.version.clone(),
            source,
            cache_file: crate::core::paths::cache_file(
                &manifest.package.name,
                &manifest.package.version,
                &detect_ext(url),
            ),
            install_dir: install_dir.clone(),
            backup_dir: install_dir.with_extension("backup"),
            product_code: None,
            dependencies: manifest.package.dependencies.clone(),
        }
    }

    #[test]
    fn install_file_name_uses_source_file_name() {
        let context = build_context("https://example.invalid/PortableApp.zip?download=1");

        assert_eq!(install_file_name(&context).unwrap(), "PortableApp.zip");
    }

    #[test]
    fn install_file_name_errors_when_source_has_no_file_name() {
        let context = build_context("");

        let error = install_file_name(&context).expect_err("missing file name should fail");

        assert!(
            error
                .to_string()
                .contains("could not determine file name from source URL")
        );
    }
}
