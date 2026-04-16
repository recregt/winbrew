PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS schema_meta (
	id INTEGER PRIMARY KEY CHECK (id = 1),
	schema_version INTEGER NOT NULL
);

INSERT OR REPLACE INTO schema_meta (id, schema_version) VALUES (1, 1);

CREATE TABLE IF NOT EXISTS release_lineage (
	hash TEXT PRIMARY KEY,
	parent_hash TEXT,
	is_snapshot INTEGER NOT NULL DEFAULT 0,
	snapshot_url TEXT,
	metadata_url TEXT,
	created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_release_lineage_parent_hash ON release_lineage(parent_hash);
CREATE INDEX IF NOT EXISTS idx_release_lineage_snapshot ON release_lineage(is_snapshot, created_at DESC);

CREATE TABLE IF NOT EXISTS patch_artifacts (
	from_hash TEXT NOT NULL,
	to_hash TEXT NOT NULL,
	file_path TEXT NOT NULL,
	size_bytes INTEGER NOT NULL,
	checksum TEXT NOT NULL,
	created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
	PRIMARY KEY (from_hash, to_hash)
);
CREATE INDEX IF NOT EXISTS idx_patch_artifacts_from_hash ON patch_artifacts(from_hash);
CREATE INDEX IF NOT EXISTS idx_patch_artifacts_to_hash ON patch_artifacts(to_hash);

CREATE TABLE IF NOT EXISTS update_plans (
	current_hash TEXT PRIMARY KEY,
	mode TEXT NOT NULL CHECK (mode IN ('current', 'full', 'patch')),
	target_hash TEXT NOT NULL,
	snapshot_url TEXT,
	patch_urls_json TEXT NOT NULL DEFAULT '[]',
	chain_length INTEGER NOT NULL DEFAULT 0,
	total_patch_bytes INTEGER NOT NULL DEFAULT 0,
	is_latest_full INTEGER NOT NULL DEFAULT 0,
	is_stale INTEGER NOT NULL DEFAULT 0,
	created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
CREATE INDEX IF NOT EXISTS idx_update_plans_latest_full ON update_plans(is_latest_full, created_at DESC);
CREATE INDEX IF NOT EXISTS idx_update_plans_mode ON update_plans(mode, is_stale);