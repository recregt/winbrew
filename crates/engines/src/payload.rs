use crate::core::{
    ArchiveKind,
    network::{installer_filename, is_zip_path},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PayloadKind {
    Raw,
    Archive(ArchiveKind),
}

pub(crate) fn classify_payload(url: &str) -> PayloadKind {
    archive_kind_for_url(url).map_or(PayloadKind::Raw, PayloadKind::Archive)
}

pub(crate) fn archive_kind_for_url(url: &str) -> Option<ArchiveKind> {
    if is_zip_path(url) {
        return Some(ArchiveKind::Zip);
    }

    let file_name = installer_filename(url).to_ascii_lowercase();
    archive_kind_from_file_name(&file_name)
}

fn archive_kind_from_file_name(file_name: &str) -> Option<ArchiveKind> {
    if file_name.ends_with(".tar.gz")
        || file_name.ends_with(".tgz")
        || file_name.ends_with(".tbz2")
        || file_name.ends_with(".tar.bz2")
        || file_name.ends_with(".tar")
    {
        Some(ArchiveKind::Tar)
    } else if file_name.ends_with(".gz") {
        Some(ArchiveKind::Gzip)
    } else if file_name.ends_with(".7z") {
        Some(ArchiveKind::SevenZip)
    } else if file_name.ends_with(".rar") {
        Some(ArchiveKind::Rar)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::{PayloadKind, archive_kind_for_url, classify_payload};
    use crate::core::ArchiveKind;

    #[test]
    fn classifies_zip_payloads_before_portable_fallback() {
        assert_eq!(
            classify_payload("https://example.invalid/tool.zip?token=123#fragment"),
            PayloadKind::Archive(ArchiveKind::Zip)
        );
    }

    #[test]
    fn classifies_non_archive_payloads_as_raw() {
        assert_eq!(
            classify_payload("https://example.invalid/tool.exe"),
            PayloadKind::Raw
        );
    }

    #[test]
    fn classifies_tar_family_payloads_as_archive() {
        assert_eq!(
            classify_payload("https://example.invalid/tool.tar.gz"),
            PayloadKind::Archive(ArchiveKind::Tar)
        );
        assert_eq!(
            classify_payload("https://example.invalid/tool.tgz"),
            PayloadKind::Archive(ArchiveKind::Tar)
        );
        assert_eq!(
            classify_payload("https://example.invalid/tool.tbz2"),
            PayloadKind::Archive(ArchiveKind::Tar)
        );
        assert_eq!(
            classify_payload("https://example.invalid/tool.tar.bz2"),
            PayloadKind::Archive(ArchiveKind::Tar)
        );
    }

    #[test]
    fn classifies_gzip_payloads_as_archive() {
        assert_eq!(
            archive_kind_for_url("https://example.invalid/tool.gz"),
            Some(ArchiveKind::Gzip)
        );
    }

    #[test]
    fn classifies_other_archive_formats_as_archive() {
        assert_eq!(
            archive_kind_for_url("https://example.invalid/tool.7z"),
            Some(ArchiveKind::SevenZip)
        );
        assert_eq!(
            archive_kind_for_url("https://example.invalid/tool.rar"),
            Some(ArchiveKind::Rar)
        );
    }
}
