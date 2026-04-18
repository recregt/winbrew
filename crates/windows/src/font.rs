use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};

use crate::registry::{register_user_font_value, unregister_user_font_value};
use windows_sys::Win32::Graphics::Gdi::{
    AddFontResourceExW, FR_NOT_ENUM, FR_PRIVATE, RemoveFontResourceExW,
};

const FONT_RESOURCE_FLAGS: u32 = FR_PRIVATE | FR_NOT_ENUM;
const SUPPORTED_FONT_EXTENSIONS: &[&str] = &["ttf", "otf", "ttc", "otc"];

/// Return the per-user Windows font directory.
pub fn user_fonts_dir() -> Result<PathBuf> {
    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .context("LOCALAPPDATA is not set on this Windows session")?;

    Ok(PathBuf::from(local_app_data)
        .join("Microsoft")
        .join("Windows")
        .join("Fonts"))
}

/// Install a raw font file into the per-user Windows font directory.
pub fn install_user_font(source_path: &Path) -> Result<PathBuf> {
    validate_font_source(source_path)?;

    let fonts_dir = user_fonts_dir()?;
    fs::create_dir_all(&fonts_dir).with_context(|| {
        format!(
            "failed to create user font directory at {}",
            fonts_dir.display()
        )
    })?;

    let file_name = source_path
        .file_name()
        .context("font source path does not have a file name")?;
    let destination = fonts_dir.join(file_name);

    fs::copy(source_path, &destination).with_context(|| {
        format!(
            "failed to copy font from {} to {}",
            source_path.display(),
            destination.display()
        )
    })?;

    let registry_value_name = register_user_font(&destination).with_context(|| {
        format!(
            "failed to register font '{}' in the Windows registry",
            destination.display()
        )
    })?;

    if let Err(err) = add_font_resource(&destination) {
        let _ = unregister_user_font_by_name(&registry_value_name);
        let _ = fs::remove_file(&destination);

        return Err(err).with_context(|| {
            format!(
                "failed to load font '{}' into the current Windows session",
                destination.display()
            )
        });
    }

    Ok(destination)
}

/// Remove a font file from the per-user Windows font directory.
pub fn remove_user_font(installed_path: &Path) -> Result<()> {
    if installed_path.as_os_str().is_empty() {
        bail!("installed font path cannot be empty");
    }

    let registry_value_name = font_registry_value_name(installed_path)?;

    let _ = remove_font_resource(installed_path);

    unregister_user_font_by_name(&registry_value_name).with_context(|| {
        format!(
            "failed to unregister font '{}' from the Windows registry",
            installed_path.display()
        )
    })?;

    match fs::remove_file(installed_path) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(err)
            .with_context(|| format!("failed to remove font at {}", installed_path.display())),
    }
}

fn register_user_font(font_path: &Path) -> Result<String> {
    let value_name = font_registry_value_name(font_path)?;
    let value_data = font_path.to_string_lossy().to_string();

    register_user_font_value(&value_name, &value_data)?;

    Ok(value_name)
}

fn unregister_user_font_by_name(value_name: &str) -> Result<()> {
    unregister_user_font_value(value_name)
}

fn font_registry_value_name(font_path: &Path) -> Result<String> {
    let file_stem = font_path
        .file_stem()
        .context("font path does not have a file stem")?
        .to_string_lossy()
        .trim()
        .to_string();

    if file_stem.is_empty() {
        bail!("font path file stem cannot be empty");
    }

    let extension = font_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();

    let Some(suffix) = font_value_suffix(&extension) else {
        bail!(
            "unsupported font extension for {}: expected one of .ttf, .otf, .ttc, or .otc",
            font_path.display()
        );
    };

    Ok(format!("{file_stem}{suffix}"))
}

fn font_value_suffix(extension: &str) -> Option<&'static str> {
    match extension {
        "ttf" | "ttc" => Some(" (TrueType)"),
        "otf" | "otc" => Some(" (OpenType)"),
        _ => None,
    }
}

fn add_font_resource(font_path: &Path) -> Result<()> {
    let wide_path = wide_path(font_path);

    let added =
        unsafe { AddFontResourceExW(wide_path.as_ptr(), FONT_RESOURCE_FLAGS, std::ptr::null()) };

    if added == 0 {
        bail!("AddFontResourceExW failed for {}", font_path.display());
    }

    Ok(())
}

fn remove_font_resource(font_path: &Path) -> Result<()> {
    let wide_path = wide_path(font_path);

    let removed =
        unsafe { RemoveFontResourceExW(wide_path.as_ptr(), FONT_RESOURCE_FLAGS, std::ptr::null()) };

    if removed == 0 {
        return Ok(());
    }

    Ok(())
}

fn wide_path(path: &Path) -> Vec<u16> {
    use std::os::windows::ffi::OsStrExt;

    let mut wide_path: Vec<u16> = path.as_os_str().encode_wide().collect();
    wide_path.push(0);
    wide_path
}

fn validate_font_source(source_path: &Path) -> Result<()> {
    if source_path.as_os_str().is_empty() {
        bail!("font source path cannot be empty");
    }

    if !source_path.exists() {
        bail!("font source path does not exist: {}", source_path.display());
    }

    if !source_path.is_file() {
        bail!("font source path is not a file: {}", source_path.display());
    }

    let extension = source_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase())
        .unwrap_or_default();

    if !SUPPORTED_FONT_EXTENSIONS.contains(&extension.as_str()) {
        bail!(
            "unsupported font extension for {}: expected one of .ttf, .otf, .ttc, or .otc",
            source_path.display()
        );
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        FONT_RESOURCE_FLAGS, font_registry_value_name, register_user_font, remove_user_font,
        unregister_user_font_by_name,
    };
    use crate::registry::user_fonts::USER_FONTS_REGISTRY_PATH;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};
    use windows_sys::Win32::Graphics::Gdi::{FR_NOT_ENUM, FR_PRIVATE};
    use winreg::{RegKey, enums::HKEY_CURRENT_USER};

    fn temp_font_path(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "winbrew-font-test-{}-{}-{name}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time should be monotonic")
                .as_nanos()
        ))
    }

    #[test]
    fn font_registry_value_name_uses_expected_suffixes() {
        assert_eq!(
            font_registry_value_name(&PathBuf::from(r"C:\Fonts\Demo.ttf"))
                .expect("ttf should parse"),
            "Demo (TrueType)"
        );
        assert_eq!(
            font_registry_value_name(&PathBuf::from(r"C:\Fonts\Demo.otf"))
                .expect("otf should parse"),
            "Demo (OpenType)"
        );
    }

    #[test]
    fn font_resource_flags_include_private_and_not_enum() {
        assert_eq!(FONT_RESOURCE_FLAGS, FR_PRIVATE | FR_NOT_ENUM);
    }

    #[test]
    fn register_and_unregister_user_font_round_trip_registry_value() {
        let font_path = temp_font_path("registry-round-trip.ttf");
        fs::write(&font_path, b"dummy font payload").expect("write temp font file");

        let value_name = register_user_font(&font_path).expect("registry entry should be written");

        let fonts_key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(USER_FONTS_REGISTRY_PATH)
            .expect("fonts key should exist");
        let stored_path: String = fonts_key
            .get_value(&value_name)
            .expect("registry value should exist");

        assert_eq!(stored_path, font_path.to_string_lossy());

        unregister_user_font_by_name(&value_name).expect("registry entry should be removed");

        let fonts_key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(USER_FONTS_REGISTRY_PATH)
            .expect("fonts key should still exist");
        assert!(fonts_key.get_value::<String, _>(&value_name).is_err());

        let _ = fs::remove_file(&font_path);
    }

    #[test]
    fn remove_user_font_cleans_registry_entry_and_file() {
        let font_path = temp_font_path("remove-round-trip.ttf");
        fs::write(&font_path, b"dummy font payload").expect("write temp font file");

        let value_name = register_user_font(&font_path).expect("registry entry should be written");
        assert!(
            RegKey::predef(HKEY_CURRENT_USER)
                .open_subkey(USER_FONTS_REGISTRY_PATH)
                .expect("fonts key should exist")
                .get_value::<String, _>(&value_name)
                .is_ok()
        );

        remove_user_font(&font_path).expect("font removal should succeed");

        assert!(!font_path.exists());
        let fonts_key = RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(USER_FONTS_REGISTRY_PATH)
            .expect("fonts key should exist");
        assert!(fonts_key.get_value::<String, _>(&value_name).is_err());
    }
}
