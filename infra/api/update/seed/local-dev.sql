PRAGMA foreign_keys = ON;

DELETE FROM update_plans;
DELETE FROM patch_artifacts;
DELETE FROM release_lineage;
DELETE FROM schema_meta;

INSERT INTO schema_meta (id, schema_version) VALUES (1, 1);

INSERT INTO update_plans (
	current_hash,
	mode,
	target_hash,
	snapshot_url,
	patch_urls_json,
	chain_length,
	total_patch_bytes,
	is_latest_full,
	is_stale
) VALUES
	('sha256:seed-current', 'current', 'sha256:seed-current', NULL, '[]', 0, 0, 0, 0),
	('sha256:seed-latest', 'full', 'sha256:seed-latest', 'https://cdn.winbrew.dev/catalog/latest.db.zst', '[]', 0, 0, 1, 0);