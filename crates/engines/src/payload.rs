use anyhow::{Context, Result};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use crate::core::{
    ArchiveKind,
    network::{installer_filename, is_zip_path},
};

mod signatures {
    pub const MSI: [u8; 8] = [0xD0, 0xCF, 0x11, 0xE0, 0xA1, 0xB1, 0x1A, 0xE1];
    pub const ZIP_LOCAL: [u8; 4] = [0x50, 0x4B, 0x03, 0x04];
    pub const ZIP_EMPTY: [u8; 4] = [0x50, 0x4B, 0x05, 0x06];
    pub const ZIP_SPANNED: [u8; 4] = [0x50, 0x4B, 0x07, 0x08];
    pub const SEVEN_ZIP: [u8; 6] = [0x37, 0x7A, 0xBC, 0xAF, 0x27, 0x1C];
    pub const GZIP: [u8; 3] = [0x1F, 0x8B, 0x08];
    pub const TAR_OFFSET: usize = 257;
    pub const TAR_MAGIC: [u8; 5] = [0x75, 0x73, 0x74, 0x61, 0x72];
    pub const CAB: [u8; 4] = [0x4D, 0x53, 0x43, 0x46];
    pub const RAR4: [u8; 7] = [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x00];
    pub const RAR5: [u8; 8] = [0x52, 0x61, 0x72, 0x21, 0x1A, 0x07, 0x01, 0x00];
    pub const MSIX_MARKERS: [&str; 2] = ["appxmanifest.xml", "appxmetadata/appxbundlemanifest.xml"];
}

/// Maximum number of bytes to read when probing a payload header.
///
/// 512 bytes is enough to cover the common signatures we use here, including
/// TAR's magic offset, without reading the entire file into memory.
const PROBE_HEADER_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PayloadKind {
    Raw,
    Archive(ArchiveKind),
    Cab,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DetectedArtifactKind {
    Msi,
    Msix,
    Archive(ArchiveKind),
    Cab,
}

pub(crate) fn classify_payload(url: &str) -> PayloadKind {
    if is_zip_path(url) {
        return PayloadKind::Archive(ArchiveKind::Zip);
    }

    let file_name = installer_filename(url).to_ascii_lowercase();

    if file_name.ends_with(".cab") {
        return PayloadKind::Cab;
    }

    archive_kind_from_file_name(&file_name).map_or(PayloadKind::Raw, PayloadKind::Archive)
}

pub(crate) fn probe_downloaded_artifact_kind(path: &Path) -> Result<Option<DetectedArtifactKind>> {
    let mut file = File::open(path)
        .with_context(|| format!("failed to open downloaded payload {}", path.display()))?;
    let mut limited_reader = file.by_ref().take(PROBE_HEADER_BYTES as u64);
    let buffer = read_probe_bytes(&mut limited_reader)?;
    file.seek(SeekFrom::Start(0))
        .with_context(|| format!("failed to rewind downloaded payload {}", path.display()))?;

    match classify_probe_bytes(&buffer) {
        Some(DetectedArtifactKind::Archive(ArchiveKind::Zip)) => try_probe_as_msix(file),
        detected => Ok(detected),
    }
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
    if is_zip_signature(bytes) {
        return Some(DetectedArtifactKind::Archive(ArchiveKind::Zip));
    }

    if is_msi_signature(bytes) {
        return Some(DetectedArtifactKind::Msi);
    }

    if is_cab_signature(bytes) {
        return Some(DetectedArtifactKind::Cab);
    }

    if is_gzip_signature(bytes) {
        return Some(DetectedArtifactKind::Archive(ArchiveKind::Gzip));
    }

    if is_tar_signature(bytes) {
        return Some(DetectedArtifactKind::Archive(ArchiveKind::Tar));
    }

    if is_seven_zip_signature(bytes) {
        return Some(DetectedArtifactKind::Archive(ArchiveKind::SevenZip));
    }

    if is_rar_signature(bytes) {
        return Some(DetectedArtifactKind::Archive(ArchiveKind::Rar));
    }

    None
}

fn try_probe_as_msix(file: File) -> Result<Option<DetectedArtifactKind>> {
    let mut archive = match zip::ZipArchive::new(file) {
        Ok(archive) => archive,
        Err(_) => return Ok(Some(DetectedArtifactKind::Archive(ArchiveKind::Zip))),
    };

    if zip_archive_looks_like_msix(&mut archive).unwrap_or(false) {
        return Ok(Some(DetectedArtifactKind::Msix));
    }

    Ok(Some(DetectedArtifactKind::Archive(ArchiveKind::Zip)))
}

fn zip_archive_looks_like_msix<R: Read + Seek>(archive: &mut zip::ZipArchive<R>) -> Result<bool> {
    for index in 0..archive.len() {
        let entry = archive.by_index(index).with_context(|| {
            format!(
                "failed to read ZIP entry {} while probing MSIX markers",
                index
            )
        })?;

        let normalized_name = entry.name().replace('\\', "/").to_ascii_lowercase();
        if signatures::MSIX_MARKERS
            .iter()
            .any(|marker| normalized_name == *marker)
        {
            return Ok(true);
        }
    }

    Ok(false)
}

fn read_probe_bytes<R: Read>(reader: &mut R) -> Result<Vec<u8>> {
    let mut buffer = Vec::with_capacity(PROBE_HEADER_BYTES);
    reader
        .read_to_end(&mut buffer)
        .context("failed to read probe bytes")?;

    Ok(buffer)
}

fn is_msi_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&signatures::MSI)
}

fn is_cab_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&signatures::CAB)
}

fn is_zip_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&signatures::ZIP_LOCAL)
        || bytes.starts_with(&signatures::ZIP_EMPTY)
        || bytes.starts_with(&signatures::ZIP_SPANNED)
}

fn is_seven_zip_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&signatures::SEVEN_ZIP)
}

fn is_gzip_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&signatures::GZIP)
}

fn is_tar_signature(bytes: &[u8]) -> bool {
    bytes
        .get(signatures::TAR_OFFSET..signatures::TAR_OFFSET + signatures::TAR_MAGIC.len())
        .is_some_and(|magic| magic == signatures::TAR_MAGIC)
}

fn is_rar_signature(bytes: &[u8]) -> bool {
    bytes.starts_with(&signatures::RAR4) || bytes.starts_with(&signatures::RAR5)
}

#[cfg(test)]
mod tests {
    use super::{
        DetectedArtifactKind, PayloadKind, archive_kind_for_url, classify_payload,
        classify_probe_bytes, probe_downloaded_artifact_kind, read_probe_bytes,
    };
    use crate::core::ArchiveKind;
    use std::fs;
    use std::io::{self, Read, Write};
    use tempfile::NamedTempFile;
    use zip::ZipWriter;
    use zip::write::SimpleFileOptions;

    struct ChunkedReader<'a> {
        bytes: &'a [u8],
        chunk_size: usize,
        offset: usize,
    }

    impl Read for ChunkedReader<'_> {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.offset >= self.bytes.len() {
                return Ok(0);
            }

            let chunk_end = (self.offset + self.chunk_size).min(self.bytes.len());
            let chunk = &self.bytes[self.offset..chunk_end];
            let count = chunk.len().min(buffer.len());
            buffer[..count].copy_from_slice(&chunk[..count]);
            self.offset += count;

            Ok(count)
        }
    }

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
    fn classifies_cab_payloads_as_cab() {
        assert_eq!(
            classify_payload("https://example.invalid/tool.cab"),
            PayloadKind::Cab
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
    fn probes_cab_signatures() {
        assert_eq!(
            classify_probe_bytes(b"MSCFcab payload"),
            Some(DetectedArtifactKind::Cab)
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
    fn classifies_empty_probe_bytes_as_none() {
        assert_eq!(classify_probe_bytes(&[]), None);
    }

    #[test]
    fn classifies_partial_probe_bytes_as_none() {
        assert_eq!(classify_probe_bytes(&[0xD0]), None);
        assert_eq!(classify_probe_bytes(b"PK\x03"), None);
    }

    #[test]
    fn probes_msix_like_zip_packages() {
        let temp_file = NamedTempFile::new().expect("temp file");
        let file = fs::File::create(temp_file.path()).expect("create zip file");
        let mut writer = ZipWriter::new(file);

        writer
            .start_file("AppxManifest.xml", SimpleFileOptions::default())
            .expect("start msix manifest entry");
        writer
            .write_all(b"<Package />")
            .expect("write msix manifest");
        writer.finish().expect("finish msix zip");

        assert_eq!(
            probe_downloaded_artifact_kind(temp_file.path()).expect("probe msix zip"),
            Some(DetectedArtifactKind::Msix)
        );
    }

    #[test]
    fn read_probe_bytes_collects_short_reads() {
        let mut reader = ChunkedReader {
            bytes: b"header-bytes",
            chunk_size: 1,
            offset: 0,
        };

        assert_eq!(
            read_probe_bytes(&mut reader).expect("read bytes"),
            b"header-bytes"
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
