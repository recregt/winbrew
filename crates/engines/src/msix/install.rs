use anyhow::{Context, Result};
use std::fs;
use std::path::Path;

#[cfg(windows)]
use windows::Management::Deployment::{AddPackageOptions, PackageManager};
#[cfg(windows)]
use windows::core::HSTRING;

use winbrew_models::{EngineInstallReceipt, EngineKind, EngineMetadata, InstallScope};

use super::installed_package_full_name;

pub fn install(
    download_path: &Path,
    install_dir: &Path,
    package_name: &str,
) -> Result<EngineInstallReceipt> {
    #[cfg(not(windows))]
    {
        let _ = (download_path, install_dir, package_name);
        anyhow::bail!("MSIX installation is only supported on Windows")
    }

    #[cfg(windows)]
    {
        let package_manager = PackageManager::new().context("failed to create package manager")?;
        let package_uri = file_uri_for_path(download_path)?;
        let options = AddPackageOptions::new().context("failed to create add package options")?;

        package_manager
            .AddPackageByUriAsync(&package_uri, &options)
            .context("failed to start msix installation")?
            .join()
            .context("msix install failed")?;

        fs::create_dir_all(install_dir)
            .with_context(|| format!("failed to create {}", install_dir.display()))?;

        let package_full_name = installed_package_full_name(package_name)?;
        let engine_metadata = Some(EngineMetadata::msix(
            package_full_name,
            InstallScope::Installed,
        ));

        Ok(EngineInstallReceipt::new(EngineKind::Msix, engine_metadata))
    }
}

#[cfg(windows)]
fn file_uri_for_path(path: &Path) -> Result<windows::Foundation::Uri> {
    let absolute_path =
        fs::canonicalize(path).with_context(|| format!("failed to resolve {}", path.display()))?;
    let file_uri = file_uri_string(&absolute_path);
    let file_uri = HSTRING::from(file_uri);

    windows::Foundation::Uri::CreateUri(&file_uri)
        .context("failed to create file URI for msix installer")
}

#[cfg(windows)]
fn file_uri_string(path: &Path) -> String {
    let path = path.to_string_lossy();
    let (scheme, path) = if let Some(path) = path.strip_prefix(r"\\?\UNC\") {
        ("file://", path)
    } else if let Some(path) = path.strip_prefix(r"\\?\") {
        ("file:///", path)
    } else if let Some(path) = path.strip_prefix(r"\\") {
        ("file://", path)
    } else {
        ("file:///", path.as_ref())
    };

    let mut file_uri = String::with_capacity(scheme.len() + path.len() + path.len() / 4);
    file_uri.push_str(scheme);
    encode_file_uri_path_into(path, &mut file_uri);

    file_uri
}

#[cfg(all(windows, test))]
fn encode_file_uri_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len() + path.len() / 4);
    encode_file_uri_path_into(path, &mut encoded);

    encoded
}

#[cfg(windows)]
fn encode_file_uri_path_into(path: &str, encoded: &mut String) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";

    for ch in path.chars() {
        if ch == '\\' {
            encoded.push('/');
        } else if is_uri_path_char(ch) {
            encoded.push(ch);
        } else {
            let mut buffer = [0u8; 4];
            for &byte in ch.encode_utf8(&mut buffer).as_bytes() {
                encoded.push('%');
                encoded.push(HEX[(byte >> 4) as usize] as char);
                encoded.push(HEX[(byte & 0x0F) as usize] as char);
            }
        }
    }
}

#[cfg(windows)]
fn is_uri_path_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-' | '.' | '_' | '~' | ':')
}

#[cfg(test)]
mod tests {
    #[cfg(windows)]
    use super::encode_file_uri_path;
    #[cfg(windows)]
    use super::file_uri_string;
    #[cfg(windows)]
    use std::path::Path;

    #[test]
    #[cfg(windows)]
    fn encode_file_uri_path_escapes_special_characters() {
        let encoded = encode_file_uri_path(r"C:\pkg\o'ne tool\app#.msix");

        assert_eq!(encoded, "C:/pkg/o%27ne%20tool/app%23.msix");
    }

    #[test]
    #[cfg(windows)]
    fn encode_file_uri_path_keeps_safe_segments() {
        let encoded = encode_file_uri_path(r"C:\Packages\Contoso.App\tool-1.0.msix");

        assert_eq!(encoded, "C:/Packages/Contoso.App/tool-1.0.msix");
    }

    #[test]
    #[cfg(windows)]
    fn file_uri_string_strips_verbatim_path_prefix() {
        let uri = file_uri_string(Path::new(r"\\?\C:\pkg\o'ne tool\app#.msix"));

        assert_eq!(uri, "file:///C:/pkg/o%27ne%20tool/app%23.msix");
    }

    #[test]
    #[cfg(windows)]
    fn file_uri_string_handles_unc_paths() {
        let uri = file_uri_string(Path::new(r"\\server\share\pkg\tool msix.appx"));

        assert_eq!(uri, "file://server/share/pkg/tool%20msix.appx");
    }
}
