use anyhow::Result;
use std::io::{Cursor, Write};

use crate::core::hash::Hasher;
use crate::models::shared::hash::HashAlgorithm;
use zip::ZipWriter;
use zip::write::SimpleFileOptions;

pub fn create_dummy_zip_bytes() -> Result<Vec<u8>> {
    let buffer = Cursor::new(Vec::new());
    let mut writer = ZipWriter::new(buffer);
    writer.start_file("bin/tool.exe", SimpleFileOptions::default())?;
    writer.write_all(b"zip-binary")?;
    let buffer = writer.finish()?;

    Ok(buffer.into_inner())
}

pub fn digest_hex(algorithm: HashAlgorithm, bytes: &[u8]) -> String {
    let mut hasher = Hasher::new(algorithm);
    hasher.update(bytes);
    let digest = hasher.finalize();

    digest.iter().map(|byte| format!("{:02x}", byte)).collect()
}

pub fn md5_hex(bytes: &[u8]) -> String {
    digest_hex(HashAlgorithm::Md5, bytes)
}

pub fn sha1_hex(bytes: &[u8]) -> String {
    digest_hex(HashAlgorithm::Sha1, bytes)
}

pub fn sha512_hex(bytes: &[u8]) -> String {
    digest_hex(HashAlgorithm::Sha512, bytes)
}
