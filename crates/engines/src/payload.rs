use anyhow::{Context, Result};
use std::fs::File;
use std::io::Read;
use std::path::Path;

use crate::core::{
    ArchiveKind,
    network::{installer_filename, is_zip_path},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PayloadKind {
    Raw,
    Archive(ArchiveKind),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DetectedArtifactKind {
    Msi,
    Archive(ArchiveKind),
}

pub(crate) fn classify_payload(url: &str) -> PayloadKind {
    archive_kind_for_url(url).map_or(PayloadKind::Raw, PayloadKind::Archive)
}

pub(crate) fn probe_downloaded_artifact_kind(path: &Path) -> Result<Option<DetectedArtifactKind>> {
    let mut file = File::open(path)
        .with_context(|| format!("failed to open downloaded payload {}", path.display()))?;
    let mut buffer = [0u8; 512];
    let read = file
        .read(&mut buffer)
        .with_context(|| format!("failed to read downloaded payload {}", path.display()))?;

    Ok(classify_probe_bytes(&buffer[..read]))
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

fn classify_probe_bytes(bytes: &[u8]) -> Option<DetectedArtifactKind> {
    if is_msi_signature(bytes) {
        return Some(DetectedArtifactKind::Msi);
    }

    if is_zip_signature(bytes) {
        return Some(DetectedArtifactKind::Archive(ArchiveKind::Zip));
    }

    if is_seven_zip_signature(bytes) {
        return Some(DetectedArtifactKind::Archive(ArchiveKind::SevenZip));
    }

    if is_gzip_signature(bytes) {
        return Some(DetectedArtifactKind::Archive(ArchiveKind::Gzip));
    }

    if is_tar_signature(bytes) {
        return Some(DetectedArtifactKind::Archive(ArchiveKind::Tar));
    }

    if is_rar_signature(bytes) {
        return Some(DetectedArtifactKind::Archive(ArchiveKind::Rar));
    }

    None
}

fn is_msi_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1])
}

fn is_zip_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
        || bytes.starts_with(b"PK\x05\x06")
        || bytes.starts_with(b"PK\x07\x08")
}

fn is_seven_zip_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C])
}

fn is_gzip_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1F, 0x8B, 0x08])
}

fn is_tar_signature(bytes: &[u8]) -> bool {
    bytes.get(257..262).is_some_and(|magic| magic == b"ustar")
}

fn is_rar_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00])
        || bytes.starts_with(&[0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00])
}

#[cfg(test)]
mod tests {
    use super::{
        DetectedArtifactKind, PayloadKind, archive_kind_for_url, classify_payload,
        classify_probe_bytes,
    };
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

    #[test]
    fn probes_msi_signatures() {
        assert_eq!(
            classify_probe_bytes(&[0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1]),
            Some(DetectedArtifactKind::Msi)
        );
    }

    #[test]
    fn probes_zip_signatures() {
        assert_eq!(
            classify_probe_bytes(b"PK\x03\x04rest"),
            Some(DetectedArtifactKind::Archive(ArchiveKind::Zip))
        );
    }

    #[test]
    fn probes_tar_signatures() {
        let mut bytes = vec![0u8; 512];
        bytes[257..262].copy_from_slice(b"ustar");

        assert_eq!(
            classify_probe_bytes(&bytes),
            Some(DetectedArtifactKind::Archive(ArchiveKind::Tar))
        );
    }
}
