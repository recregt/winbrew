use std::fs;
use std::io::{Read, Write};
use std::path::Path;

use crate::fs::{FsError, Result};

use super::super::platform::PlatformAdapter;
use super::{ExtractionContext, ExtractionLimits};

pub(crate) fn extract_zip_archive_with_platform<P: PlatformAdapter>(
    zip_path: &Path,
    destination_dir: &Path,
    limits: ExtractionLimits,
) -> Result<()> {
    let file = fs::File::open(zip_path).map_err(|err| FsError::open_zip_archive(zip_path, err))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|err| FsError::open_zip_archive(zip_path, err))?;
    const ZIP_COPY_BUFFER_SIZE: usize = 256 * 1024;
    let mut extraction = ExtractionContext::<P>::new(limits);
    let mut buffer = vec![0u8; ZIP_COPY_BUFFER_SIZE];

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|err| FsError::read_zip_entry(zip_path, err))?;
        extract_entry(&mut entry, destination_dir, &mut extraction, &mut buffer)?;
    }

    extraction.commit();
    Ok(())
}

fn extract_entry<P: PlatformAdapter, R: Read>(
    entry: &mut zip::read::ZipFile<'_, R>,
    destination_dir: &Path,
    extraction: &mut ExtractionContext<P>,
    buffer: &mut [u8],
) -> Result<()> {
    let enclosed_name = entry
        .enclosed_name()
        .ok_or_else(FsError::invalid_zip_entry_path)?;

    if entry.is_symlink() {
        return Err(FsError::symlink_entry(
            &destination_dir.join(&enclosed_name),
        ));
    }

    let outpath = destination_dir.join(&enclosed_name);

    extraction.validate_target(&outpath, destination_dir)?;

    extraction.check_limits(&enclosed_name, entry.size(), entry.compressed_size())?;

    if entry.is_dir() {
        extraction.ensure_directory_tree(&outpath)?;
        return Ok(());
    }

    if let Some(parent) = outpath.parent() {
        extraction.ensure_directory_tree(parent)?;
    }

    extraction.validate_target(&outpath, destination_dir)?;

    let mut outfile = P::create_extracted_file(&outpath)
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
