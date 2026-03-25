#[path = "common/mod.rs"]
mod common;

use common::{TestEnvVar, env_lock, fixtures, mock_server::MockServer};
use tempfile::tempdir;
use winbrew::database;
use winbrew::sources::SourceAdapter;
use winbrew::sources::winget::WingetSource;

#[test]
fn code_search_returns_manifest_candidates() -> anyhow::Result<()> {
    let _guard = env_lock();
    let temp_root = tempdir()?;
    let _root_env = TestEnvVar::set("WINBREW_ROOT", temp_root.path().to_string_lossy().as_ref());

    fixtures::init_database_root(temp_root.path())?;

    let mut server = MockServer::new();
    let manifest = fixtures::winget_fixture("windows-terminal.installer.yaml")?;

    database::config_set("core.github_token", "secret-token")?;
    database::config_set("sources.winget.repo_slug", "microsoft/winget-pkgs")?;
    database::config_set("sources.winget.url", &server.url())?;
    database::config_set("sources.winget.api_base", &server.url())?;
    database::config_set("sources.winget.format", "yaml")?;

    let search_mock = server.get_json_with_query(
        "/search/code",
        mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded(
                "q".into(),
                r#"repo:microsoft/winget-pkgs "windows terminal" "ManifestType: installer" path:manifests/"#.into(),
            ),
            mockito::Matcher::UrlEncoded("per_page".into(), "10".into()),
        ]),
        r#"{"items":[{"path":"manifests/m/Microsoft/WindowsTerminal/1.21.2361.0/Microsoft.WindowsTerminal.installer.yaml"}]}"#,
    );

    let manifest_mock = server.get_text(
        "/manifests/m/Microsoft/WindowsTerminal/1.21.2361.0/Microsoft.WindowsTerminal.installer.yaml",
        manifest.as_str(),
    );

    let source = WingetSource;
    let results = source.search_packages("windows terminal")?;

    search_mock.assert();
    manifest_mock.assert();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].identifier, "Microsoft.WindowsTerminal");
    assert_eq!(results[0].package_name.as_deref(), Some("Windows Terminal"));
    assert_eq!(results[0].version, "1.21.2361.0");
    assert_eq!(
        results[0].manifest_path.as_deref(),
        Some(
            "manifests/m/Microsoft/WindowsTerminal/1.21.2361.0/Microsoft.WindowsTerminal.installer.yaml"
        )
    );

    Ok(())
}

#[test]
fn code_search_forbidden_falls_back_to_repository_contents() -> anyhow::Result<()> {
    let _guard = env_lock();
    let temp_root = tempdir()?;
    let _root_env = TestEnvVar::set("WINBREW_ROOT", temp_root.path().to_string_lossy().as_ref());

    fixtures::init_database_root(temp_root.path())?;

    let mut server = MockServer::new();
    let manifest = fixtures::winget_fixture("windows-terminal.installer.yaml")?;
    let base_url = server.url();

    database::config_set("core.github_token", "secret-token")?;
    database::config_set("sources.winget.repo_slug", "microsoft/winget-pkgs")?;
    database::config_set("sources.winget.url", &server.url())?;
    database::config_set("sources.winget.api_base", &server.url())?;
    database::config_set("sources.winget.format", "yaml")?;

    let search_mock = server.get_json_with_query_status(
        "/search/code",
        mockito::Matcher::AllOf(vec![
            mockito::Matcher::UrlEncoded(
                "q".into(),
                r#"repo:microsoft/winget-pkgs "windows terminal" "ManifestType: installer" path:manifests/"#.into(),
            ),
            mockito::Matcher::UrlEncoded("per_page".into(), "10".into()),
        ]),
        403,
        r#"{"message":"forbidden"}"#,
    );

    let root_listing = server.get_json_with_query(
        "/repos/microsoft/winget-pkgs/contents/manifests",
        mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
            "ref".into(),
            "master".into(),
        )]),
        &format!(
            r#"[{{"name":"m","path":"manifests/m","type":"dir","url":"{base}/repos/microsoft/winget-pkgs/contents/manifests/m"}}]"#,
            base = base_url
        ),
    );

    let partition_listing = server.get_json(
        "/repos/microsoft/winget-pkgs/contents/manifests/m",
        &format!(
            r#"[{{"name":"Microsoft","path":"manifests/m/Microsoft","type":"dir","url":"{base}/repos/microsoft/winget-pkgs/contents/manifests/m/Microsoft"}}]"#,
            base = base_url
        ),
    );

    let publisher_listing = server.get_json(
        "/repos/microsoft/winget-pkgs/contents/manifests/m/Microsoft",
        &format!(
            r#"[{{"name":"WindowsTerminal","path":"manifests/m/Microsoft/WindowsTerminal","type":"dir","url":"{base}/repos/microsoft/winget-pkgs/contents/manifests/m/Microsoft/WindowsTerminal"}}]"#,
            base = base_url
        ),
    );

    let package_listing = server.get_json(
        "/repos/microsoft/winget-pkgs/contents/manifests/m/Microsoft/WindowsTerminal",
        &format!(
            r#"[{{"name":"1.21.2361.0","path":"manifests/m/Microsoft/WindowsTerminal/1.21.2361.0","type":"dir","url":"{base}/repos/microsoft/winget-pkgs/contents/manifests/m/Microsoft/WindowsTerminal/1.21.2361.0"}}]"#,
            base = base_url
        ),
    );

    let manifest_mock = server.get_text(
        "/manifests/m/Microsoft/WindowsTerminal/1.21.2361.0/Microsoft.WindowsTerminal.installer.yaml",
        manifest.as_str(),
    );

    let source = WingetSource;
    let results = source.search_packages("windows terminal")?;

    search_mock.assert();
    root_listing.assert();
    partition_listing.assert();
    publisher_listing.assert();
    package_listing.assert();
    manifest_mock.assert();

    assert_eq!(results.len(), 1);
    assert_eq!(results[0].identifier, "Microsoft.WindowsTerminal");
    assert_eq!(results[0].package_name.as_deref(), Some("Windows Terminal"));

    Ok(())
}

#[test]
fn fallback_search_walks_repository_contents() -> anyhow::Result<()> {
    let _guard = env_lock();
    let temp_root = tempdir()?;
    let _root_env = TestEnvVar::set("WINBREW_ROOT", temp_root.path().to_string_lossy().as_ref());

    fixtures::init_database_root(temp_root.path())?;

    let mut server = MockServer::new();
    let manifest = fixtures::winget_fixture("windows-terminal.installer.yaml")?;
    let base_url = server.url();

    database::config_set("sources.winget.url", &server.url())?;
    database::config_set("sources.winget.api_base", &server.url())?;
    database::config_set("sources.winget.format", "yaml")?;
    database::config_set("core.github_token", "")?;

    let root_listing = server.get_json_with_query(
        "/repos/microsoft/winget-pkgs/contents/manifests",
        mockito::Matcher::AllOf(vec![mockito::Matcher::UrlEncoded(
            "ref".into(),
            "master".into(),
        )]),
        &format!(
            r#"[{{"name":"m","path":"manifests/m","type":"dir","url":"{base}/repos/microsoft/winget-pkgs/contents/manifests/m"}}]"#,
            base = base_url
        ),
    );

    let partition_listing = server.get_json(
        "/repos/microsoft/winget-pkgs/contents/manifests/m",
        &format!(
            r#"[{{"name":"Microsoft","path":"manifests/m/Microsoft","type":"dir","url":"{base}/repos/microsoft/winget-pkgs/contents/manifests/m/Microsoft"}}]"#,
            base = base_url
        ),
    );

    let publisher_listing = server.get_json(
        "/repos/microsoft/winget-pkgs/contents/manifests/m/Microsoft",
        &format!(
            r#"[{{"name":"WindowsTerminal","path":"manifests/m/Microsoft/WindowsTerminal","type":"dir","url":"{base}/repos/microsoft/winget-pkgs/contents/manifests/m/Microsoft/WindowsTerminal"}}]"#,
            base = base_url
        ),
    );

    let package_listing = server.get_json(
        "/repos/microsoft/winget-pkgs/contents/manifests/m/Microsoft/WindowsTerminal",
        &format!(
            r#"[{{"name":"1.21.2361.0","path":"manifests/m/Microsoft/WindowsTerminal/1.21.2361.0","type":"dir","url":"{base}/repos/microsoft/winget-pkgs/contents/manifests/m/Microsoft/WindowsTerminal/1.21.2361.0"}}]"#,
            base = base_url
        ),
    );

    let manifest_mock = server.get_text(
        "/manifests/m/Microsoft/WindowsTerminal/1.21.2361.0/Microsoft.WindowsTerminal.installer.yaml",
        manifest.as_str(),
    );

    let source = WingetSource;
    let results = source.search_packages("windows terminal")?;

    root_listing.assert();
    partition_listing.assert();
    publisher_listing.assert();
    package_listing.assert();
    manifest_mock.assert();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].identifier, "Microsoft.WindowsTerminal");
    assert_eq!(results[0].package_name.as_deref(), Some("Windows Terminal"));

    Ok(())
}
