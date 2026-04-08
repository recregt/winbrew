pub fn installer_filename(url: &str) -> String {
    url_path(url)
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or("download.bin")
        .to_string()
}

pub fn is_zip_path(url: &str) -> bool {
    let path = url_path(url);

    path.rsplit_once('.')
        .is_some_and(|(_, ext)| ext.eq_ignore_ascii_case("zip"))
}

fn url_path(url: &str) -> &str {
    let path = url.split('#').next().unwrap_or(url);
    path.split('?').next().unwrap_or(path)
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
    fn is_zip_path_ignores_query_string() {
        assert!(is_zip_path("https://example.invalid/tool.zip?token=abc"));
        assert!(!is_zip_path("https://example.invalid/tool.exe?token=abc"));
    }
}
