use std::fs;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use flate2::read::GzDecoder;

use crate::fs::{FsError, Result};

pub(crate) fn extract_gzip_archive(gzip_path: &Path, destination_dir: &Path) -> Result<()> {
    let file = fs::File::open(gzip_path).map_err(|err| FsError::open_archive(gzip_path, err))?;
    let mut decoder = GzDecoder::new(file);

    fs::create_dir_all(destination_dir)
        .map_err(|err| FsError::create_directory(destination_dir, err))?;

    let output_path = output_path_for_gzip_archive(gzip_path, destination_dir)?;
    let temp_output_path = temporary_output_path_for(&output_path);
    let mut output_file = fs::File::create(&temp_output_path)
        .map_err(|err| FsError::create_extracted_file(&temp_output_path, err))?;

    let copy_result = copy_gzip_contents(
        gzip_path,
        &output_path,
        &temp_output_path,
        &mut decoder,
        &mut output_file,
    );

    drop(output_file);

    if let Err(err) = copy_result {
        let _ = fs::remove_file(&temp_output_path);
        return Err(err);
    }

    fs::rename(&temp_output_path, &output_path)
        .map_err(|err| FsError::finalize_file(&temp_output_path, &output_path, err))?;

    Ok(())
}

fn copy_gzip_contents(
    gzip_path: &Path,
    output_path: &Path,
    temp_output_path: &Path,
    decoder: &mut GzDecoder<fs::File>,
    output_file: &mut fs::File,
) -> Result<()> {
    let mut buffer = [0u8; 8 * 1024];

    loop {
        let bytes_read = decoder
            .read(&mut buffer)
            .map_err(|err| FsError::read_archive_entry(gzip_path, err))?;

        if bytes_read == 0 {
            break;
        }

        output_file
            .write_all(&buffer[..bytes_read])
            .map_err(|err| FsError::write_entry(output_path, err))?;
    }

    output_file
        .sync_all()
        .map_err(|err| FsError::sync_temp_file(temp_output_path, err))?;

    Ok(())
}

fn output_path_for_gzip_archive(gzip_path: &Path, destination_dir: &Path) -> Result<PathBuf> {
    let file_name = gzip_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            FsError::open_archive(
                gzip_path,
                io::Error::new(
                    io::ErrorKind::InvalidInput,
                    "gzip archive path has no file name",
                ),
            )
        })?;

    let lower_file_name = file_name.to_ascii_lowercase();
    if !lower_file_name.ends_with(".gz") {
        return Err(FsError::open_archive(
            gzip_path,
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "gzip archive path does not end with .gz",
            ),
        ));
    }

    let output_name = &file_name[..file_name.len() - 3];
    if output_name.is_empty() {
        return Err(FsError::open_archive(
            gzip_path,
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "gzip archive path resolves to an empty output name",
            ),
        ));
    }

    Ok(destination_dir.join(output_name))
}

fn temporary_output_path_for(output_path: &Path) -> PathBuf {
    let file_name = output_path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("output");

    output_path.with_file_name(format!("{file_name}.tmp"))
}
