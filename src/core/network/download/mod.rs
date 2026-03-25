use anyhow::Result;
use std::path::Path;

use crate::core::fs::TempFileGuard;
use crate::core::network::http::NetworkSettings;

mod request;
mod stream;

pub use request::{open_target, send_request};
pub use stream::{stream_response, verify_download};

pub fn download<F>(settings: &NetworkSettings, url: &str, dest: &Path, on_progress: F) -> Result<()>
where
    F: FnMut(u64, u64),
{
    download_inner(settings, url, dest, on_progress, None)
}

pub fn download_and_verify<F>(
    settings: &NetworkSettings,
    url: &str,
    dest: &Path,
    checksum: &str,
    on_progress: F,
) -> Result<()>
where
    F: FnMut(u64, u64),
{
    download_inner(settings, url, dest, on_progress, Some(checksum))
}

fn download_inner<F>(
    settings: &NetworkSettings,
    url: &str,
    dest: &Path,
    mut on_progress: F,
    expected_checksum: Option<&str>,
) -> Result<()>
where
    F: FnMut(u64, u64),
{
    let mut response = send_request(settings, url, dest)?;
    let mut temp_guard = TempFileGuard::new(dest.with_extension("part"));

    {
        let mut target = open_target(dest, &response)?;

        if target.existing_size > 0 {
            on_progress(target.existing_size, target.total_size);
        }

        let hasher = stream_response(
            &mut response,
            &mut target,
            expected_checksum,
            &mut on_progress,
        )?;

        verify_download(&target, hasher, expected_checksum)?;

        target.finalize(dest)?;
        temp_guard.keep();
        Ok(())
    }
}
