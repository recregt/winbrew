use sha2::{Digest, Sha256};
use tempfile::tempdir;
use winbrew::core::network::{NetworkSettings, download_and_verify};

#[test]
fn download_and_verify_fetches_from_mock_server() {
    let mut server = mockito::Server::new();
    let body = b"portable package contents";
    let checksum = hex::encode(Sha256::digest(body));
    let mock = server
        .mock("GET", "/portable.zip")
        .with_status(200)
        .with_header("content-length", body.len().to_string().as_str())
        .with_body(body.as_ref())
        .create();

    let temp_dir = tempdir().expect("temporary directory should be created");
    let dest = temp_dir.path().join("portable.zip");
    let settings = NetworkSettings {
        timeout_secs: 5,
        proxy_url: None,
        github_token: None,
    };
    let url = format!("{}/portable.zip", server.url());

    download_and_verify(&settings, &url, &dest, &checksum, |_, _| {})
        .expect("download should succeed");

    mock.assert();
    assert_eq!(
        std::fs::read(&dest).expect("downloaded file should exist"),
        body
    );
    assert!(!dest.with_extension("part").exists());
}
