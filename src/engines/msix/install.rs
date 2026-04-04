use anyhow::{Context, Result};
use std::fs;
use std::path::Path;
use windows::Management::Deployment::{AddPackageOptions, PackageManager};
use windows::core::HSTRING;

pub fn install(download_path: &Path, install_dir: &Path) -> Result<()> {
    let package_manager = PackageManager::new().context("failed to create package manager")?;
    let package_uri = file_uri_for_path(download_path)?;
    let options = AddPackageOptions::new().context("failed to create add package options")?;

    package_manager
        .AddPackageByUriAsync(&package_uri, &options)
        .context("failed to start msix installation")?
        .get()
        .context("msix install failed")?;

    fs::create_dir_all(install_dir)
        .with_context(|| format!("failed to create {}", install_dir.display()))?;

    Ok(())
}

fn file_uri_for_path(path: &Path) -> Result<windows::Foundation::Uri> {
    let absolute_path =
        fs::canonicalize(path).with_context(|| format!("failed to resolve {}", path.display()))?;
    let file_uri = format!("file:///{}", encode_file_uri_path(&absolute_path));
    let file_uri = HSTRING::from(file_uri);

    windows::Foundation::Uri::CreateUri(&file_uri)
        .context("failed to create file URI for msix installer")
}

fn encode_file_uri_path(path: &Path) -> String {
    let path = path.to_string_lossy().replace('\\', "/");
    let mut encoded = String::with_capacity(path.len() * 3);

    for ch in path.chars() {
        if is_uri_path_char(ch) {
            encoded.push(ch);
        } else {
            let mut buffer = [0; 4];
            for byte in ch.encode_utf8(&mut buffer).as_bytes() {
                encoded.push('%');
                encoded.push_str(&format!("{:02X}", byte));
            }
        }
    }

    encoded
}

fn is_uri_path_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || matches!(ch, '/' | '-' | '.' | '_' | '~' | ':')
}

#[cfg(test)]
mod tests {
    use super::encode_file_uri_path;
    use std::path::Path;

    #[test]
    fn encode_file_uri_path_escapes_special_characters() {
        let encoded = encode_file_uri_path(Path::new(r"C:\pkg\o'ne tool\app#.msix"));

        assert_eq!(encoded, "C:/pkg/o%27ne%20tool/app%23.msix");
    }

    #[test]
    fn encode_file_uri_path_keeps_safe_segments() {
        let encoded = encode_file_uri_path(Path::new(r"C:\Packages\Contoso.App\tool-1.0.msix"));

        assert_eq!(encoded, "C:/Packages/Contoso.App/tool-1.0.msix");
    }
}
