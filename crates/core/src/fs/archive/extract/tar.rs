use std::fs;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use bzip2::read::BzDecoder;
use flate2::read::GzDecoder;

use crate::fs::{FsError, Result};

use super::super::platform::PlatformAdapter;
use super::{ExtractionContext, ExtractionLimits};

const TAR_COPY_BUFFER_SIZE: usize = 256 * 1024;

pub(crate) fn extract_tar_archive_with_platform<P: PlatformAdapter>(
    archive_path: &Path,
    destination_dir: &Path,
    limits: ExtractionLimits,
) -> Result<()> {
    let archive_file =
        fs::File::open(archive_path).map_err(|err| FsError::open_archive(archive_path, err))?;
    let archive_size = fs::metadata(archive_path)
        .map_err(|err| FsError::open_archive(archive_path, err))?
        .len();
    let reader = archive_reader_for_path(archive_path, archive_file);
    let mut archive = tar::Archive::new(reader);
    let mut extraction = ExtractionContext::<P>::new(limits);
    let mut buffer = vec![0u8; TAR_COPY_BUFFER_SIZE];

    let entries = archive
        .entries()
        .map_err(|err| FsError::read_archive_entry(archive_path, err))?;

    for entry in entries {
        let mut entry = entry.map_err(|err| FsError::read_archive_entry(archive_path, err))?;
        extract_entry(
            &mut entry,
            archive_size,
            destination_dir,
            &mut extraction,
            &mut buffer,
        )?;
    }

    extraction.commit();
    Ok(())
}

fn archive_reader_for_path(archive_path: &Path, file: fs::File) -> Box<dyn Read> {
    let file_name = archive_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();

    if file_name.ends_with(".tar.gz") || file_name.ends_with(".tgz") {
        Box::new(GzDecoder::new(file))
    } else if file_name.ends_with(".tbz2") || file_name.ends_with(".tar.bz2") {
        Box::new(BzDecoder::new(file))
    } else {
        Box::new(file)
    }
}

fn extract_entry<P: PlatformAdapter, R: Read>(
    entry: &mut tar::Entry<'_, R>,
    archive_size: u64,
    destination_dir: &Path,
    extraction: &mut ExtractionContext<P>,
    buffer: &mut [u8],
) -> Result<()> {
    let entry_path = entry
        .path()
        .map_err(|_| FsError::invalid_archive_entry_path())?;
    let enclosed_name = sanitize_entry_path(entry_path.as_ref())?;

    let outpath = destination_dir.join(&enclosed_name);

    extraction.validate_target(&outpath, destination_dir)?;
    extraction.check_limits(&enclosed_name, entry.size(), archive_size)?;

    let entry_type = entry.header().entry_type();

    if entry_type.is_symlink() {
        return Err(FsError::symlink_entry(&outpath));
    }

    if entry_type.is_hard_link() {
        return Err(FsError::unsupported_entry(&outpath));
    }

    if entry_type.is_dir() {
        extraction.ensure_directory_tree(&outpath)?;
        return Ok(());
    }

    if !entry_type.is_file() {
        return Err(FsError::unsupported_entry(&outpath));
    }

    if let Some(parent) = outpath.parent() {
        extraction.ensure_directory_tree(parent)?;
    }

    let mut outfile = P::create_extraction_target_file(&outpath)
        .map_err(|err| FsError::create_extracted_file(&outpath, err))?;
    extraction.record_file(&outpath);

    loop {
        let bytes_read = entry
            .read(buffer)
            .map_err(|err| FsError::read_entry(&outpath, err))?;
        if bytes_read == 0 {
            break;
        }

        outfile
            .write_all(&buffer[..bytes_read])
            .map_err(|err| FsError::write_entry(&outpath, err))?;
    }

    Ok(())
}

fn sanitize_entry_path(path: &Path) -> Result<PathBuf> {
    let mut enclosed = PathBuf::new();

    for component in path.components() {
        match component {
            Component::Normal(part) => enclosed.push(part),
            Component::CurDir => {}
            _ => return Err(FsError::invalid_archive_entry_path()),
        }
    }

    if enclosed.as_os_str().is_empty() {
        return Err(FsError::invalid_archive_entry_path());
    }

    Ok(enclosed)
}

#[cfg(test)]
mod tests {
    use crate::fs::archive::{ArchiveKind, extract_archive};
    use std::fs;
    use tempfile::tempdir;

    fn create_tar_archive(path: &std::path::Path, file_name: &str, contents: &[u8]) {
        let file = fs::File::create(path).expect("create tar file");
        let mut builder = tar::Builder::new(file);
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();

        builder
            .append_data(&mut header, file_name, contents)
            .expect("append tar entry");
        builder.finish().expect("finish tar file");
    }

    fn create_tar_gz_archive(path: &std::path::Path, file_name: &str, contents: &[u8]) {
        let file = fs::File::create(path).expect("create tar.gz file");
        let encoder = flate2::write::GzEncoder::new(file, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();

        builder
            .append_data(&mut header, file_name, contents)
            .expect("append tar.gz entry");
        let encoder = builder.into_inner().expect("finish tar builder");
        encoder.finish().expect("finish tar.gz file");
    }

    fn create_tar_bz2_archive(path: &std::path::Path, file_name: &str, contents: &[u8]) {
        let file = fs::File::create(path).expect("create tar.bz2 file");
        let encoder = bzip2::write::BzEncoder::new(file, bzip2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_size(contents.len() as u64);
        header.set_mode(0o644);
        header.set_cksum();

        builder
            .append_data(&mut header, file_name, contents)
            .expect("append tar.bz2 entry");
        let encoder = builder.into_inner().expect("finish tar builder");
        encoder.finish().expect("finish tar.bz2 file");
    }

    #[test]
    fn extract_tar_archive_extracts_plain_tar() {
        let temp_dir = tempdir().expect("temp dir");
        let destination_dir = temp_dir.path().join("dest");
        let archive_path = temp_dir.path().join("archive.tar");

        fs::create_dir_all(&destination_dir).expect("destination dir");
        create_tar_archive(&archive_path, "bin/tool.exe", b"tar payload");

        extract_archive(ArchiveKind::Tar, &archive_path, &destination_dir).expect("tar extraction");

        assert_eq!(
            fs::read(destination_dir.join("bin/tool.exe")).expect("read"),
            b"tar payload"
        );
    }

    #[test]
    fn extract_tar_archive_extracts_tar_gz() {
        let temp_dir = tempdir().expect("temp dir");
        let destination_dir = temp_dir.path().join("dest");
        let archive_path = temp_dir.path().join("archive.tar.gz");

        fs::create_dir_all(&destination_dir).expect("destination dir");
        create_tar_gz_archive(&archive_path, "bin/tool.exe", b"tar gz payload");

        extract_archive(ArchiveKind::Tar, &archive_path, &destination_dir)
            .expect("tar.gz extraction");

        assert_eq!(
            fs::read(destination_dir.join("bin/tool.exe")).expect("read"),
            b"tar gz payload"
        );
    }

    #[test]
    fn extract_tar_archive_extracts_tar_bz2() {
        let temp_dir = tempdir().expect("temp dir");
        let destination_dir = temp_dir.path().join("dest");
        let archive_path = temp_dir.path().join("archive.tar.bz2");

        fs::create_dir_all(&destination_dir).expect("destination dir");
        create_tar_bz2_archive(&archive_path, "bin/tool.exe", b"tar bz2 payload");

        extract_archive(ArchiveKind::Tar, &archive_path, &destination_dir)
            .expect("tar.bz2 extraction");

        assert_eq!(
            fs::read(destination_dir.join("bin/tool.exe")).expect("read"),
            b"tar bz2 payload"
        );
    }
}
