/// Returns the last URL path segment or `download.bin` when the URL does not expose one.
pub fn installer_filename(url: &str) -> String {
    last_path_segment(url).unwrap_or_else(|| "download.bin".to_string())
}

/// Returns `true` when the URL path ends in `.zip`, ignoring query and fragment parts.
pub fn is_zip_path(url: &str) -> bool {
    last_path_segment(url).is_some_and(|segment| {
        segment
            .rsplit_once('.')
            .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("zip"))
    })
}

fn last_path_segment(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;

    parsed
        .path_segments()?
        .next_back()
        .filter(|segment| !segment.is_empty())
        .map(str::to_string)
}

#[cfg(test)]
mod tests {
    use super::{installer_filename, is_zip_path};

    #[test]
    fn installer_filename_uses_last_segment() {
        assert_eq!(
            installer_filename("https://example.invalid/a/b/tool.zip"),
            "tool.zip"
        );
    }

    #[test]
    fn installer_filename_ignores_query_and_fragment() {
        assert_eq!(
            installer_filename("https://example.invalid/tool.exe?token=123#xyz"),
            "tool.exe"
        );
    }

    #[test]
    fn installer_filename_falls_back_when_last_segment_is_empty() {
        assert_eq!(
            installer_filename("https://example.invalid/downloads/"),
            "download.bin"
        );
    }

    #[test]
    fn is_zip_path_ignores_query_string() {
        assert!(is_zip_path("https://example.invalid/tool.zip?token=abc"));
        assert!(!is_zip_path("https://example.invalid/tool.exe?token=abc"));
    }

    #[test]
    fn is_zip_path_rejects_empty_last_segment() {
        assert!(!is_zip_path("https://example.invalid/downloads/"));
    }

    #[test]
    fn is_zip_path_is_case_insensitive() {
        assert!(is_zip_path("https://example.invalid/tool.ZIP"));
        assert!(is_zip_path("https://example.invalid/tool.Zip"));
    }
}
