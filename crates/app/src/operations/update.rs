//! Catalog refresh workflow for the CLI.

use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::fs;
use std::fs::File;
use std::io::{Cursor, Read};
use std::path::Path;
use zstd::stream::read::Decoder;

use crate::core::fs::cleanup_path;
use crate::core::fs::finalize_temp_file;
use crate::core::hash::{hash_file, verify_hash};
use crate::core::network::{Client, build_client, download_url_to_temp_file};
use crate::core::paths::{ResolvedPaths, ensure_dirs_at};
use crate::models::catalog::CatalogMetadata;
use crate::models::domains::shared::HashAlgorithm;

const CATALOG_UPDATE_API_URL: &str = "https://api.winbrew.dev/v1/update";
const CATALOG_DIRECT_DOWNLOAD_URL: &str = "https://wb-assets.recregt.com/catalog.db";
const CATALOG_METADATA_DIRECT_DOWNLOAD_URL: &str = "https://wb-assets.recregt.com/metadata.json";

#[derive(Debug, Clone, Deserialize)]
struct CatalogUpdateResponse {
    mode: CatalogUpdateMode,
    current: String,
    target: String,
    snapshot: Option<String>,
    #[serde(default)]
    patches: Vec<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum CatalogUpdateMode {
    Full,
    Patch,
}

#[derive(Debug, Clone)]
enum CatalogDownloadPlan {
    Full {
        catalog_url: String,
        metadata_url: String,
        expected_hash: Option<String>,
    },
    Patch {
        patch_urls: Vec<String>,
        expected_hash: String,
    },
}

pub fn refresh_catalog<FStart, FProgress>(
    paths: &ResolvedPaths,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    refresh_catalog_with_api_url(paths, CATALOG_UPDATE_API_URL, on_start, on_progress)
}

fn refresh_catalog_with_api_url<FStart, FProgress>(
    paths: &ResolvedPaths,
    update_api_url: &str,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let catalog_path = paths.catalog_db.clone();
    let catalog_dir = catalog_path
        .parent()
        .context("failed to resolve catalog database directory")?;

    ensure_dirs_at(&paths.root).context("failed to create catalog directories")?;

    let catalog_temp_path = catalog_dir.join("catalog.db.download");
    let metadata_temp_path = catalog_dir.join("metadata.json.download");
    let metadata_path = catalog_dir.join("metadata.json");

    let result = (|| -> Result<()> {
        clear_temp_file(&catalog_temp_path)?;
        clear_temp_file(&metadata_temp_path)?;

        let client = build_client("winbrew-catalog-downloader")?;

        let local_metadata = load_local_catalog_metadata(&metadata_path)?;
        let download_plan =
            resolve_catalog_download_plan(&client, update_api_url, local_metadata.as_ref())?;

        match &download_plan {
            CatalogDownloadPlan::Full { .. } => {
                download_catalog_release(
                    &client,
                    &download_plan,
                    &catalog_temp_path,
                    &metadata_temp_path,
                    on_start,
                    on_progress,
                )?;
            }
            CatalogDownloadPlan::Patch {
                patch_urls,
                expected_hash,
            } => {
                let previous_metadata = local_metadata
                    .as_ref()
                    .context("patch updates require local catalog metadata")?;

                if let Err(err) = apply_catalog_patch_release(
                    &client,
                    &catalog_path,
                    &catalog_temp_path,
                    &metadata_temp_path,
                    patch_urls,
                    expected_hash,
                    previous_metadata.current_hash.as_str(),
                ) {
                    tracing::warn!(error = %err, "patch catalog update failed; falling back to full snapshot");
                    clear_temp_file(&catalog_temp_path)?;
                    clear_temp_file(&metadata_temp_path)?;
                    download_catalog_release(
                        &client,
                        &CatalogDownloadPlan::Full {
                            catalog_url: CATALOG_DIRECT_DOWNLOAD_URL.to_string(),
                            metadata_url: CATALOG_METADATA_DIRECT_DOWNLOAD_URL.to_string(),
                            expected_hash: Some(expected_hash.clone()),
                        },
                        &catalog_temp_path,
                        &metadata_temp_path,
                        on_start,
                        on_progress,
                    )?;
                }
            }
        }

        finalize_temp_file(&catalog_temp_path, &catalog_path)?;
        finalize_temp_file(&metadata_temp_path, &metadata_path)?;

        Ok(())
    })();

    let _ = cleanup_path(&catalog_temp_path);
    let _ = cleanup_path(&metadata_temp_path);

    result
}

fn clear_temp_file(path: &Path) -> Result<()> {
    cleanup_path(path).context("failed to clear previous catalog download")
}

fn load_local_catalog_metadata(path: &Path) -> Result<Option<CatalogMetadata>> {
    match fs::metadata(path) {
        Ok(_) => load_catalog_metadata(path).map(Some),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err).context("failed to inspect local catalog metadata"),
    }
}

fn resolve_catalog_download_plan(
    client: &Client,
    update_api_url: &str,
    local_metadata: Option<&CatalogMetadata>,
) -> Result<CatalogDownloadPlan> {
    if let Some(plan) =
        resolve_catalog_download_plan_via_api(client, update_api_url, local_metadata)?
    {
        return Ok(plan);
    }

    Ok(CatalogDownloadPlan::Full {
        catalog_url: CATALOG_DIRECT_DOWNLOAD_URL.to_string(),
        metadata_url: CATALOG_METADATA_DIRECT_DOWNLOAD_URL.to_string(),
        expected_hash: None,
    })
}

fn resolve_catalog_download_plan_via_api(
    client: &Client,
    update_api_url: &str,
    local_metadata: Option<&CatalogMetadata>,
) -> Result<Option<CatalogDownloadPlan>> {
    let api_url = local_metadata
        .map(|metadata| format!("{update_api_url}?current={}", metadata.current_hash))
        .unwrap_or_else(|| update_api_url.to_string());

    let request = client.get(api_url);

    let response = match request.send() {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };

    let response = match response.error_for_status() {
        Ok(response) => response,
        Err(_) => return Ok(None),
    };

    let selection: CatalogUpdateResponse = match response.json() {
        Ok(selection) => selection,
        Err(_) => return Ok(None),
    };

    if selection.target.trim().is_empty() {
        return Ok(None);
    }

    match selection.mode {
        CatalogUpdateMode::Full => {
            let catalog_url = match selection.snapshot {
                Some(snapshot) if !snapshot.trim().is_empty() => snapshot,
                _ => return Ok(None),
            };

            let metadata_url = metadata_url_for_snapshot_url(&catalog_url)?;

            Ok(Some(CatalogDownloadPlan::Full {
                catalog_url,
                metadata_url,
                expected_hash: Some(selection.target),
            }))
        }
        CatalogUpdateMode::Patch => {
            let local_metadata = match local_metadata {
                Some(metadata) => metadata,
                None => return Ok(None),
            };

            if local_metadata.current_hash != selection.current {
                return Ok(None);
            }

            if selection.patches.is_empty()
                || selection.patches.iter().any(|url| url.trim().is_empty())
            {
                return Ok(None);
            }

            Ok(Some(CatalogDownloadPlan::Patch {
                patch_urls: selection.patches,
                expected_hash: selection.target,
            }))
        }
    }
}

fn download_catalog_release<FStart, FProgress>(
    client: &Client,
    plan: &CatalogDownloadPlan,
    catalog_temp_path: &Path,
    metadata_temp_path: &Path,
    on_start: FStart,
    on_progress: FProgress,
) -> Result<()>
where
    FStart: FnOnce(Option<u64>),
    FProgress: FnMut(u64),
{
    let CatalogDownloadPlan::Full {
        catalog_url,
        metadata_url,
        expected_hash,
    } = plan
    else {
        bail!("download_catalog_release only supports full snapshot plans");
    };

    download_url_to_temp_file(
        client,
        metadata_url,
        metadata_temp_path,
        "catalog metadata asset",
        |_| {},
        |_| {},
        |_| Ok(()),
    )?;

    let metadata = load_catalog_metadata(metadata_temp_path)?;

    if let Some(expected_hash) = expected_hash
        && metadata.current_hash.as_str() != expected_hash.as_str()
    {
        bail!(
            "catalog metadata hash mismatch: expected {expected_hash}, got {}",
            metadata.current_hash
        );
    }

    download_url_to_temp_file(
        client,
        catalog_url,
        catalog_temp_path,
        "catalog asset",
        on_start,
        on_progress,
        |_| Ok(()),
    )
    .context("failed to download catalog asset")?;

    verify_catalog_hash(catalog_temp_path, &metadata.current_hash)?;

    Ok(())
}

fn apply_catalog_patch_release(
    client: &Client,
    catalog_path: &Path,
    catalog_temp_path: &Path,
    metadata_temp_path: &Path,
    patch_urls: &[String],
    expected_hash: &str,
    previous_hash: &str,
) -> Result<()> {
    if !catalog_path.exists() {
        bail!("cannot apply catalog patch without an existing catalog database");
    }

    fs::copy(catalog_path, catalog_temp_path)
        .context("failed to back up local catalog database for patch update")?;

    let connection =
        Connection::open(catalog_temp_path).context("failed to open catalog patch working copy")?;
    connection
        .pragma_update(None, "journal_mode", "DELETE")
        .context("failed to set catalog patch journal mode")?;
    connection
        .execute_batch("PRAGMA foreign_keys = ON;")
        .context("failed to enable foreign keys for catalog patch update")?;

    for patch_url in patch_urls {
        let patch_sql = download_catalog_patch_sql(client, patch_url)?;
        connection
            .execute_batch(&patch_sql)
            .with_context(|| format!("failed to apply catalog patch from {patch_url}"))?;
    }

    let integrity_check: String = connection
        .query_row("PRAGMA integrity_check", [], |row| row.get(0))
        .context("failed to run catalog integrity check after patch application")?;

    if integrity_check.trim() != "ok" {
        bail!("catalog integrity check failed after patch application: {integrity_check}");
    }

    let metadata =
        build_catalog_metadata_from_connection(&connection, expected_hash, previous_hash)?;

    drop(connection);

    verify_catalog_hash(catalog_temp_path, &metadata.current_hash)?;

    fs::write(
        metadata_temp_path,
        serde_json::to_vec_pretty(&metadata)
            .context("failed to serialize patched catalog metadata")?,
    )
    .context("failed to write patched catalog metadata")?;

    Ok(())
}

fn download_catalog_patch_sql(client: &Client, patch_url: &str) -> Result<String> {
    let response = client
        .get(patch_url.to_string())
        .send()
        .with_context(|| format!("failed to send catalog patch request to {patch_url}"))?;
    let response = response
        .error_for_status()
        .with_context(|| format!("catalog patch request failed for {patch_url}"))?;

    let patch_bytes = response
        .bytes()
        .with_context(|| format!("failed to read catalog patch response from {patch_url}"))?;

    let mut decoder = Decoder::new(Cursor::new(patch_bytes))
        .context("failed to decompress catalog patch payload")?;
    let mut patch_sql = String::new();
    decoder
        .read_to_string(&mut patch_sql)
        .context("failed to decode catalog patch SQL")?;

    Ok(patch_sql)
}

fn build_catalog_metadata_from_connection(
    connection: &Connection,
    current_hash: &str,
    previous_hash: &str,
) -> Result<CatalogMetadata> {
    let package_count: i64 = connection
        .query_row("SELECT COUNT(*) FROM catalog_packages", [], |row| {
            row.get(0)
        })
        .context("failed to count catalog packages")?;
    let package_count =
        usize::try_from(package_count).context("catalog package count does not fit in usize")?;

    let mut source_counts = BTreeMap::new();
    let mut stmt = connection
        .prepare(
            "SELECT source, COUNT(*) FROM catalog_packages GROUP BY source ORDER BY source ASC",
        )
        .context("failed to prepare catalog source count query")?;
    let mut rows = stmt
        .query([])
        .context("failed to query catalog source counts")?;

    while let Some(row) = rows
        .next()
        .context("failed to read catalog source count row")?
    {
        let source: String = row.get(0).context("failed to read catalog source name")?;
        let count: i64 = row.get(1).context("failed to read catalog source count")?;
        let count = usize::try_from(count).context("catalog source count does not fit in usize")?;
        source_counts.insert(source, count);
    }

    let mut metadata =
        CatalogMetadata::build_from_counts(package_count, source_counts, current_hash.to_string());
    metadata.previous_hash = previous_hash.to_string();
    metadata.validate()?;

    Ok(metadata)
}

fn metadata_url_for_snapshot_url(snapshot_url: &str) -> Result<String> {
    let (prefix, _) = snapshot_url
        .rsplit_once('/')
        .context("snapshot URL must contain a path segment")?;

    Ok(format!("{prefix}/metadata.json"))
}

fn load_catalog_metadata(path: &Path) -> Result<CatalogMetadata> {
    let file = File::open(path).context("failed to open catalog metadata download")?;
    let metadata: CatalogMetadata =
        serde_json::from_reader(file).context("failed to decode catalog metadata download")?;
    metadata.validate()?;

    Ok(metadata)
}

fn verify_catalog_hash(path: &Path, expected_hash: &str) -> Result<()> {
    let actual_hash = hash_file(path, HashAlgorithm::Sha256)
        .context("failed to hash downloaded catalog database")?;

    verify_hash(expected_hash, actual_hash).map_err(Into::into)
}

#[cfg(test)]
mod tests {
    use super::{
        load_catalog_metadata, metadata_url_for_snapshot_url, refresh_catalog_with_api_url,
        verify_catalog_hash,
    };
    use crate::core::hash::Hasher;
    use crate::core::paths::resolved_paths;
    use crate::models::catalog::CatalogMetadata;
    use crate::models::domains::shared::HashAlgorithm;
    use rusqlite::Connection;
    use std::collections::BTreeMap;
    use std::fs;
    use std::io::Cursor;
    use std::path::Path;
    use tempfile::tempdir;
    use winbrew_testing::{Matcher, MockServer};
    use zstd::stream::encode_all;

    const CATALOG_SCHEMA: &str = include_str!("../../../../infra/parser/schema/catalog.sql");

    fn sha256_hex(bytes: &[u8]) -> String {
        let mut hasher = Hasher::new(HashAlgorithm::Sha256);
        hasher.update(bytes);

        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    fn seed_catalog_database(path: &Path) {
        let connection = Connection::open(path).expect("open catalog database");
        connection
            .execute_batch(CATALOG_SCHEMA)
            .expect("load catalog schema");
    }

    fn apply_catalog_patch_sql(path: &Path, patch_sql: &str) {
        let connection = Connection::open(path).expect("open catalog database");
        connection
            .pragma_update(None, "journal_mode", "DELETE")
            .expect("set journal mode");
        connection
            .execute_batch("PRAGMA foreign_keys = ON;")
            .expect("enable foreign keys");
        connection
            .execute_batch(patch_sql)
            .expect("apply catalog patch sql");
    }

    fn file_hash(path: &Path) -> String {
        let bytes = fs::read(path).expect("read file for hashing");
        format!("sha256:{}", sha256_hex(&bytes))
    }

    #[test]
    fn load_catalog_metadata_reads_valid_metadata() {
        let temp_dir = tempdir().expect("temp dir");
        let path = temp_dir.path().join("metadata.json");
        let metadata = CatalogMetadata::build_from_counts(
            2,
            BTreeMap::from([(String::from("scoop"), 1)]),
            String::from("sha256:abc"),
        );

        fs::write(
            &path,
            serde_json::to_vec_pretty(&metadata).expect("serialize metadata"),
        )
        .expect("write metadata");

        let loaded = load_catalog_metadata(&path).expect("load metadata");

        assert_eq!(loaded.current_hash, metadata.current_hash);
        assert_eq!(loaded.package_count, metadata.package_count);
        assert_eq!(loaded.source_counts.get("scoop"), Some(&1));
    }

    #[test]
    fn verify_catalog_hash_accepts_matching_hash() {
        let temp_dir = tempdir().expect("temp dir");
        let path = temp_dir.path().join("catalog.db");
        let contents = b"catalog-bytes";

        fs::write(&path, contents).expect("write catalog");

        let expected_hash = format!("sha256:{}", sha256_hex(contents));

        verify_catalog_hash(&path, &expected_hash).expect("hash should match");
    }

    #[test]
    fn verify_catalog_hash_rejects_mismatch() {
        let temp_dir = tempdir().expect("temp dir");
        let path = temp_dir.path().join("catalog.db");

        fs::write(&path, b"catalog-bytes").expect("write catalog");

        let err = verify_catalog_hash(
            &path,
            "sha256:0000000000000000000000000000000000000000000000000000000000000000",
        )
        .expect_err("hash mismatch should fail");

        assert!(err.to_string().contains("checksum mismatch"));
    }

    #[test]
    fn metadata_url_is_derived_from_snapshot_url() {
        assert_eq!(
            metadata_url_for_snapshot_url("https://cdn.example.invalid/releases/catalog.db")
                .expect("metadata url should be derived"),
            "https://cdn.example.invalid/releases/metadata.json"
        );
    }

    #[test]
    fn refresh_catalog_applies_api_selected_patches() {
        let temp_dir = tempdir().expect("temp dir");
        let root = temp_dir.path();
        let paths = resolved_paths(
            root,
            "${root}/packages",
            "${root}/data",
            "${root}/data/logs",
            "${root}/data/cache",
        );

        let catalog_dir = paths
            .catalog_db
            .parent()
            .expect("catalog dir should exist")
            .to_path_buf();
        fs::create_dir_all(&catalog_dir).expect("catalog dir should be created");

        seed_catalog_database(&paths.catalog_db);

        let current_hash = file_hash(&paths.catalog_db);
        let previous_metadata =
            CatalogMetadata::build_from_counts(0, BTreeMap::new(), current_hash.clone());
        fs::write(
            catalog_dir.join("metadata.json"),
            serde_json::to_vec_pretty(&previous_metadata).expect("serialize previous metadata"),
        )
        .expect("write previous metadata");

        let patch_sql = r#"
            INSERT INTO catalog_packages (
                id, name, version, source, namespace, source_id, description, homepage, license, publisher, locale, moniker, tags, bin, created_at, updated_at
            ) VALUES (
                'winget/Contoso.App',
                'Contoso App',
                '1.2.3',
                'winget',
                NULL,
                'Contoso.App',
                'Example package',
                NULL,
                NULL,
                'Contoso Ltd.',
                'en-US',
                'contoso',
                NULL,
                NULL,
                '2026-04-14 12:00:00',
                '2026-04-14 12:34:56'
            );
        "#;

        let expected_catalog_path = catalog_dir.join("expected-catalog.db");
        fs::copy(&paths.catalog_db, &expected_catalog_path).expect("copy catalog database");
        apply_catalog_patch_sql(&expected_catalog_path, patch_sql);
        let target_hash = file_hash(&expected_catalog_path);

        let patch_payload =
            encode_all(Cursor::new(patch_sql.as_bytes()), 0).expect("compress patch sql");

        let mut server = MockServer::new();
        let patch_url = format!("{}/patches/0001.sql.zst", server.url());
        let api_response = serde_json::json!({
            "mode": "patch",
            "current": current_hash,
            "target": target_hash,
            "snapshot": null,
            "patches": [patch_url]
        });

        let _api_mock = server.mock_get_with_query(
            "/v1/update",
            Matcher::UrlEncoded(
                "current".to_string(),
                previous_metadata.current_hash.clone(),
            ),
            serde_json::to_vec(&api_response).expect("serialize api response"),
        );
        let _patch_mock = server.mock_get("/patches/0001.sql.zst", patch_payload);

        refresh_catalog_with_api_url(
            &paths,
            &format!("{}/v1/update", server.url()),
            |_| {},
            |_| {},
        )
        .expect("refresh should succeed");

        let downloaded_catalog = fs::read(&paths.catalog_db).expect("read downloaded catalog");
        assert_eq!(file_hash(&paths.catalog_db), target_hash);
        assert_eq!(
            downloaded_catalog,
            fs::read(&expected_catalog_path).expect("read expected catalog")
        );

        let downloaded_metadata: CatalogMetadata = serde_json::from_slice(
            &fs::read(catalog_dir.join("metadata.json")).expect("read downloaded metadata"),
        )
        .expect("decode downloaded metadata");

        assert_eq!(downloaded_metadata.current_hash, target_hash);
        assert_eq!(downloaded_metadata.previous_hash, current_hash);
        assert_eq!(downloaded_metadata.package_count, 1);
        assert_eq!(downloaded_metadata.source_counts.get("winget"), Some(&1));
    }

    #[test]
    fn refresh_catalog_uses_api_selected_snapshot() {
        let temp_dir = tempdir().expect("temp dir");
        let root = temp_dir.path();
        let paths = resolved_paths(
            root,
            "${root}/packages",
            "${root}/data",
            "${root}/data/logs",
            "${root}/data/cache",
        );

        let catalog_dir = paths
            .catalog_db
            .parent()
            .expect("catalog dir should exist")
            .to_path_buf();
        fs::create_dir_all(&catalog_dir).expect("catalog dir should be created");

        let previous_metadata = CatalogMetadata::build_from_counts(
            1,
            BTreeMap::from([(String::from("winget"), 1)]),
            String::from("sha256:previous"),
        );
        fs::write(
            catalog_dir.join("metadata.json"),
            serde_json::to_vec_pretty(&previous_metadata).expect("serialize previous metadata"),
        )
        .expect("write previous metadata");

        let catalog_bytes = b"catalog-bytes".to_vec();
        let catalog_hash = format!("sha256:{}", sha256_hex(&catalog_bytes));

        let mut server = MockServer::new();
        let snapshot_url = format!("{}/catalog.db", server.url());
        let api_response = serde_json::json!({
            "mode": "full",
            "current": "sha256:previous",
            "target": catalog_hash,
            "snapshot": snapshot_url,
            "patches": []
        });

        let _api_mock = server.mock_get_with_query(
            "/v1/update",
            Matcher::UrlEncoded("current".to_string(), "sha256:previous".to_string()),
            serde_json::to_vec(&api_response).expect("serialize api response"),
        );
        let _catalog_mock = server.mock_get("/catalog.db", catalog_bytes.clone());
        let metadata = CatalogMetadata::build_from_counts(
            1,
            BTreeMap::from([(String::from("winget"), 1)]),
            catalog_hash.clone(),
        );
        let _metadata_mock = server.mock_get(
            "/metadata.json",
            serde_json::to_vec_pretty(&metadata).expect("serialize metadata"),
        );

        refresh_catalog_with_api_url(
            &paths,
            &format!("{}/v1/update", server.url()),
            |_| {},
            |_| {},
        )
        .expect("refresh should succeed");

        let downloaded_catalog = fs::read(&paths.catalog_db).expect("read downloaded catalog");
        assert_eq!(downloaded_catalog, catalog_bytes);

        let downloaded_metadata: CatalogMetadata = serde_json::from_slice(
            &fs::read(catalog_dir.join("metadata.json")).expect("read downloaded metadata"),
        )
        .expect("decode downloaded metadata");

        assert_eq!(downloaded_metadata.current_hash, catalog_hash);
    }
}
