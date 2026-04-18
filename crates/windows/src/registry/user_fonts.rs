use anyhow::{Context, Result};
use winreg::{
    RegKey,
    enums::{HKEY_CURRENT_USER, KEY_SET_VALUE},
};

pub(crate) const USER_FONTS_REGISTRY_PATH: &str =
    r"Software\Microsoft\Windows NT\CurrentVersion\Fonts";

/// Write a per-user font registry entry.
pub(crate) fn register_user_font_value(value_name: &str, value_data: &str) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);
    let (fonts_key, _) = hkcu
        .create_subkey(USER_FONTS_REGISTRY_PATH)
        .context("failed to create Windows fonts registry key")?;

    fonts_key
        .set_value(value_name, &value_data)
        .with_context(|| format!("failed to write registry entry '{}'", value_name))?;

    Ok(())
}

/// Remove a per-user font registry entry.
pub(crate) fn unregister_user_font_value(value_name: &str) -> Result<()> {
    let hkcu = RegKey::predef(HKEY_CURRENT_USER);

    let Ok(fonts_key) = hkcu.open_subkey_with_flags(USER_FONTS_REGISTRY_PATH, KEY_SET_VALUE) else {
        return Ok(());
    };

    match fonts_key.delete_value(value_name) {
        Ok(()) => Ok(()),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => {
            Err(err).with_context(|| format!("failed to remove registry entry '{}'", value_name))
        }
    }
}
