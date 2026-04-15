use anyhow::{Context, Result, bail};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

static SYSTEM_FONT_PATH: OnceLock<PathBuf> = OnceLock::new();

/// Return a supported font file from the Windows Fonts directory.
pub fn system_font_path() -> Result<PathBuf> {
    if let Some(path) = SYSTEM_FONT_PATH.get() {
        return Ok(path.clone());
    }

    let path = discover_system_font_path()?;
    Ok(SYSTEM_FONT_PATH.get_or_init(|| path.clone()).clone())
}

/// Build a safe filename for a font fixture using the discovered system font extension.
pub fn system_font_file_name(prefix: &str) -> Result<String> {
    let font_path = system_font_path()?;
    let extension = font_path
        .extension()
        .and_then(|ext| ext.to_str())
        .context("system font path should have a file extension")?;

    Ok(format!("{}.{}", prefix, extension.to_ascii_lowercase()))
}

fn discover_system_font_path() -> Result<PathBuf> {
    let windows_root = std::env::var_os("WINDIR")
        .or_else(|| std::env::var_os("SystemRoot"))
        .context("WINDIR/SystemRoot is not set on this Windows session")?;
    let fonts_dir = PathBuf::from(windows_root).join("Fonts");

    if !fonts_dir.is_dir() {
        bail!(
            "Windows Fonts directory does not exist: {}",
            fonts_dir.display()
        );
    }

    let mut candidates = Vec::new();
    for entry in fs::read_dir(&fonts_dir).with_context(|| {
        format!(
            "failed to read Windows Fonts directory at {}",
            fonts_dir.display()
        )
    })? {
        let path = entry?.path();
        if path.is_file() && is_supported_font_file(&path) {
            candidates.push(path);
        }
    }

    candidates.sort_by_key(|path| font_sort_key(path));

    candidates.into_iter().next().with_context(|| {
        format!(
            "no supported system font files found in {}",
            fonts_dir.display()
        )
    })
}

fn font_sort_key(path: &Path) -> (u8, String) {
    let extension_rank = match path
        .extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.to_ascii_lowercase())
        .as_deref()
    {
        Some("ttf") => 0,
        Some("otf") => 1,
        Some("ttc") => 2,
        Some("otc") => 3,
        _ => 4,
    };

    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    (extension_rank, file_name)
}

fn is_supported_font_file(path: &Path) -> bool {
    matches!(
        path.extension()
            .and_then(|ext| ext.to_str())
            .map(|ext| ext.to_ascii_lowercase())
            .as_deref(),
        Some("ttf") | Some("otf") | Some("ttc") | Some("otc")
    )
}
