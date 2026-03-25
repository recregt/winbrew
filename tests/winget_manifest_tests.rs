#[path = "common/mod.rs"]
mod common;

use common::{TestEnvVar, env_lock, fixtures, mock_server::MockServer};
use tempfile::tempdir;
use winbrew::database;
use winbrew::sources::SourceAdapter;
use winbrew::sources::winget::WingetSource;

#[test]
fn parses_winget_yaml_fixture_via_public_source_api() -> anyhow::Result<()> {
    let _guard = env_lock();
    let temp_root = tempdir()?;
    let _root_env = TestEnvVar::set("WINBREW_ROOT", temp_root.path().to_string_lossy().as_ref());

    fixtures::init_database_root(temp_root.path())?;

    let mut server = MockServer::new();
    let manifest = fixtures::winget_fixture("windows-terminal.installer.yaml")?;

    database::config_set("sources.winget.url", &server.url())?;
    database::config_set("sources.winget.format", "yaml")?;

    let manifest_mock = server.get_text(
        "/manifests/m/Microsoft/WindowsTerminal/1.21.2361.0/Microsoft.WindowsTerminal.installer.yaml",
        manifest.as_str(),
    );

    let conn = database::get_conn()?;
    let source = WingetSource;
    let parsed = source.fetch_manifest(&conn, "Microsoft.WindowsTerminal", "1.21.2361.0")?;

    manifest_mock.assert();

    assert_eq!(parsed.package.name, "Microsoft.WindowsTerminal");
    assert_eq!(
        parsed.package.package_name.as_deref(),
        Some("Windows Terminal")
    );
    assert_eq!(parsed.package.version, "1.21.2361.0");
    assert_eq!(
        parsed.package.publisher.as_deref(),
        Some("Microsoft Corporation")
    );
    assert_eq!(
        parsed.package.description.as_deref(),
        Some("Open source terminal application for developers.")
    );
    assert_eq!(parsed.installers.len(), 1);
    assert_eq!(parsed.installers[0].to_source().kind, "msix");
    assert_eq!(
        parsed.installers[0].display_name.as_deref(),
        Some("Windows Terminal")
    );

    Ok(())
}
