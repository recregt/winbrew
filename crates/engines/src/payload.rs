use winbrew_core::network::{installer_filename, is_zip_path};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PortablePayloadKind {
    Raw,
    ZipArchive,
    UnsupportedArchive { format: String },
}

pub(crate) fn classify_portable_payload(url: &str) -> PortablePayloadKind {
    if is_zip_path(url) {
        return PortablePayloadKind::ZipArchive;
    }

    let file_name = installer_filename(url).to_ascii_lowercase();

    match unsupported_archive_format(&file_name) {
        Some(format) => PortablePayloadKind::UnsupportedArchive {
            format: format.to_string(),
        },
        None => PortablePayloadKind::Raw,
    }
}

fn unsupported_archive_format(file_name: &str) -> Option<&'static str> {
    if file_name.ends_with(".tar.gz") {
        Some("tar.gz")
    } else if file_name.ends_with(".tgz") {
        Some("tgz")
    } else if file_name.ends_with(".tbz2") {
        Some("tbz2")
    } else if file_name.ends_with(".7z") {
        Some("7z")
    } else if file_name.ends_with(".rar") {
        Some("rar")
    } else if file_name.ends_with(".tar") {
        Some("tar")
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{PortablePayloadKind, classify_portable_payload};

    #[test]
    fn classifies_zip_payloads_before_portable_fallback() {
        assert_eq!(
            classify_portable_payload("https://example.invalid/tool.zip?token=123#fragment"),
            PortablePayloadKind::ZipArchive
        );
    }

    #[test]
    fn classifies_raw_payloads_as_portable() {
        assert_eq!(
            classify_portable_payload("https://example.invalid/tool.exe"),
            PortablePayloadKind::Raw
        );
    }

    #[test]
    fn classifies_known_archive_formats_as_unsupported() {
        assert_eq!(
            classify_portable_payload("https://example.invalid/tool.7z"),
            PortablePayloadKind::UnsupportedArchive {
                format: "7z".to_string(),
            }
        );
        assert_eq!(
            classify_portable_payload("https://example.invalid/tool.tar.gz"),
            PortablePayloadKind::UnsupportedArchive {
                format: "tar.gz".to_string(),
            }
        );
        assert_eq!(
            classify_portable_payload("https://example.invalid/tool.rar"),
            PortablePayloadKind::UnsupportedArchive {
                format: "rar".to_string(),
            }
        );
    }
}
