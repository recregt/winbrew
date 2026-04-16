//! Catalog refresh workflow for the CLI.
//!
//! The update selector stays API-driven. If a patch chain fails, the workflow
//! re-queries the API for a full snapshot plan instead of falling back to a
//! hardcoded bucket URL.

mod api;
mod download;
mod metadata;
mod patch;
mod planner;
mod types;

use anyhow::{Context, Result, bail};
use std::path::Path;

use self::types::CatalogDownloadPlan;

use crate::core::fs::{cleanup_path, finalize_temp_file};
use crate::core::network::{Client, build_client};
use crate::core::paths::{ResolvedPaths, ensure_dirs_at};

const CATALOG_UPDATE_API_URL: &str = "https://api.winbrew.dev/v1/update";

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
        let local_metadata = metadata::load_local_catalog_metadata(&metadata_path)?;

        let selection =
            api::fetch_catalog_update_selection(&client, update_api_url, local_metadata.as_ref())?;
        let download_plan =
            match planner::plan_catalog_download(local_metadata.as_ref(), selection)? {
                Some(plan) => plan,
                None => request_full_snapshot_plan(&client, update_api_url)?,
            };

        match &download_plan {
            CatalogDownloadPlan::Current {
                current_hash,
                target_hash,
            } => {
                if current_hash != target_hash {
                    tracing::warn!(current_hash = %current_hash, target_hash = %target_hash, "update worker reported a current plan with mismatched hashes");
                }

                return Ok(());
            }
            CatalogDownloadPlan::Full { .. } => {
                download::download_catalog_release(
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

                if let Err(err) = patch::apply_catalog_patch_release(
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

                    let fallback_plan = request_full_snapshot_plan(&client, update_api_url)?;
                    download::download_catalog_release(
                        &client,
                        &fallback_plan,
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

fn request_full_snapshot_plan(
    client: &Client,
    update_api_url: &str,
) -> Result<CatalogDownloadPlan> {
    let selection = api::fetch_full_snapshot_update_selection(client, update_api_url)?;

    match planner::plan_catalog_download(None, selection)? {
        Some(plan @ CatalogDownloadPlan::Full { .. }) => Ok(plan),
        _ => bail!("update API did not return a full snapshot plan"),
    }
}

fn clear_temp_file(path: &Path) -> Result<()> {
    cleanup_path(path).context("failed to clear previous catalog download")
}

#[cfg(test)]
mod tests {
    use super::refresh_catalog_with_api_url;
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

    use super::metadata::{
        load_catalog_metadata, metadata_url_for_snapshot_url, verify_catalog_hash,
    };

    const CATALOG_SCHEMA: &str = include_str!("../../../../../infra/parser/schema/catalog.sql");

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
    fn refresh_catalog_noops_when_api_reports_current_state() {
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

        let mut server = MockServer::new();
        let api_response = serde_json::json!({
            "mode": "current",
            "current": current_hash,
            "target": current_hash,
            "snapshot": null,
            "patches": []
        });

        let _api_mock = server.mock_get_with_query(
            "/v1/update",
            Matcher::UrlEncoded(
                "current".to_string(),
                previous_metadata.current_hash.clone(),
            ),
            serde_json::to_vec(&api_response).expect("serialize api response"),
        );

        refresh_catalog_with_api_url(
            &paths,
            &format!("{}/v1/update", server.url()),
            |_| {},
            |_| {},
        )
        .expect("refresh should succeed");

        assert_eq!(file_hash(&paths.catalog_db), current_hash);

        let downloaded_metadata: CatalogMetadata = serde_json::from_slice(
            &fs::read(catalog_dir.join("metadata.json")).expect("read downloaded metadata"),
        )
        .expect("decode downloaded metadata");

        assert_eq!(
            downloaded_metadata.current_hash,
            previous_metadata.current_hash
        );
        assert_eq!(
            downloaded_metadata.package_count,
            previous_metadata.package_count
        );
    }

    #[test]
    fn refresh_catalog_requeries_api_for_full_snapshot_after_patch_failure() {
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

        let expected_patch_catalog_path = catalog_dir.join("expected-patch-catalog.db");
        fs::copy(&paths.catalog_db, &expected_patch_catalog_path).expect("copy catalog database");
        apply_catalog_patch_sql(&expected_patch_catalog_path, patch_sql);
        let patch_target_hash = file_hash(&expected_patch_catalog_path);

        let full_catalog_bytes = b"full-catalog-bytes".to_vec();
        let full_catalog_hash = format!("sha256:{}", sha256_hex(&full_catalog_bytes));
        let full_metadata = CatalogMetadata::build_from_counts(
            1,
            BTreeMap::from([(String::from("winget"), 1)]),
            full_catalog_hash.clone(),
        );

        let mut server = MockServer::new();
        let patch_url = format!("{}/patches/0001.sql.zst", server.url());
        let patch_api_response = serde_json::json!({
            "mode": "patch",
            "current": current_hash.clone(),
            "target": patch_target_hash,
            "snapshot": null,
            "patches": [patch_url]
        });
        let full_snapshot_url = format!("{}/releases/v2.2.0/catalog.db", server.url());
        let full_api_response = serde_json::json!({
            "mode": "full",
            "current": current_hash,
            "target": full_catalog_hash.clone(),
            "snapshot": full_snapshot_url,
            "patches": []
        });

        let _patch_api_mock = server.mock_get_with_query(
            "/v1/update",
            Matcher::UrlEncoded(
                "current".to_string(),
                previous_metadata.current_hash.clone(),
            ),
            serde_json::to_vec(&patch_api_response).expect("serialize patch api response"),
        );
        let _patch_mock = server.mock_get_with_status("/patches/0001.sql.zst", 500, "boom");
        let _full_api_mock = server.mock_get(
            "/v1/update",
            serde_json::to_vec(&full_api_response).expect("serialize full api response"),
        );
        let _catalog_mock =
            server.mock_get("/releases/v2.2.0/catalog.db", full_catalog_bytes.clone());
        let _metadata_mock = server.mock_get(
            "/releases/v2.2.0/metadata.json",
            serde_json::to_vec_pretty(&full_metadata).expect("serialize full metadata"),
        );

        refresh_catalog_with_api_url(
            &paths,
            &format!("{}/v1/update", server.url()),
            |_| {},
            |_| {},
        )
        .expect("refresh should succeed");

        let downloaded_catalog = fs::read(&paths.catalog_db).expect("read downloaded catalog");
        assert_eq!(downloaded_catalog, full_catalog_bytes);
        assert_eq!(file_hash(&paths.catalog_db), full_catalog_hash);

        let downloaded_metadata: CatalogMetadata = serde_json::from_slice(
            &fs::read(catalog_dir.join("metadata.json")).expect("read downloaded metadata"),
        )
        .expect("decode downloaded metadata");

        assert_eq!(downloaded_metadata.current_hash, full_catalog_hash);
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
