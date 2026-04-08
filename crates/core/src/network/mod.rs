mod download;
mod error;

pub use download::{
    Client, build_client, download_url_to_temp_file, installer_filename, is_zip_path,
};
pub use error::{BoxError, DownloadError, Result};
