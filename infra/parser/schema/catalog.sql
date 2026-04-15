-- Canonical catalog schema for parser-generated snapshots.
-- Parser code and tests include this file directly to avoid schema drift.
PRAGMA user_version = 2;

CREATE TABLE IF NOT EXISTS schema_meta (
    name  TEXT PRIMARY KEY,
    value TEXT NOT NULL
);

INSERT OR REPLACE INTO schema_meta (name, value)
VALUES ('schema_version', '2');

CREATE TABLE IF NOT EXISTS catalog_packages (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    version     TEXT NOT NULL,
    source      TEXT NOT NULL CHECK (source IN ('winget', 'scoop', 'chocolatey', 'winbrew')),
    namespace   TEXT CHECK (namespace IS NULL OR length(trim(namespace)) > 0),
    source_id   TEXT NOT NULL CHECK (length(trim(source_id)) > 0),
    description TEXT,
    homepage    TEXT,
    license     TEXT,
    publisher   TEXT,
    created_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at  TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS catalog_installers (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    package_id  TEXT NOT NULL REFERENCES catalog_packages(id) ON DELETE CASCADE,
    -- Current sources require a direct download URL.
    url         TEXT NOT NULL,
    -- Checksum is optional for checksumless manifests.
    hash        TEXT,
    hash_algorithm TEXT NOT NULL DEFAULT 'sha256' CHECK (hash_algorithm IN ('md5', 'sha1', 'sha256', 'sha512')),
    -- Normalized installer family used for filtering and source-aware browse flows.
    installer_type TEXT NOT NULL DEFAULT 'unknown' CHECK (installer_type IN ('msi', 'msix', 'appx', 'exe', 'inno', 'nullsoft', 'wix', 'burn', 'pwa', 'font', 'portable', 'zip', 'msstore', 'nuget', 'scoop', 'unknown')),
    installer_switches TEXT,
    -- Winget manifest scope when the source provides one.
    scope       TEXT CHECK (scope IS NULL OR scope IN ('machine', 'user')),
    arch        TEXT NOT NULL DEFAULT '',
    -- Raw installer format used by the engine-facing model.
    kind        TEXT NOT NULL DEFAULT '',
    nested_kind TEXT
);

CREATE TABLE IF NOT EXISTS catalog_packages_raw (
    package_id  TEXT PRIMARY KEY REFERENCES catalog_packages(id) ON DELETE CASCADE,
    raw         TEXT CHECK (raw IS NULL OR json_valid(raw))
);

CREATE VIRTUAL TABLE IF NOT EXISTS catalog_packages_fts USING fts5(
    name,
    description,
    content=catalog_packages,
    content_rowid=rowid
);

CREATE INDEX IF NOT EXISTS idx_catalog_packages_name    ON catalog_packages(name);
CREATE INDEX IF NOT EXISTS idx_catalog_installers_pkg   ON catalog_installers(package_id);
CREATE UNIQUE INDEX IF NOT EXISTS idx_catalog_packages_identity ON catalog_packages(
    source,
    IFNULL(namespace, ''),
    source_id
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_catalog_installers_unique ON catalog_installers(
    package_id,
    url,
    IFNULL(hash, ''),
    hash_algorithm,
    installer_type,
    IFNULL(installer_switches, ''),
    IFNULL(scope, ''),
    arch,
    kind,
    IFNULL(nested_kind, '')
);

CREATE TRIGGER IF NOT EXISTS catalog_packages_ai AFTER INSERT ON catalog_packages BEGIN
    INSERT INTO catalog_packages_fts(rowid, name, description)
    VALUES (new.rowid, new.name, COALESCE(new.description, ''));
END;

CREATE TRIGGER IF NOT EXISTS catalog_packages_ad AFTER DELETE ON catalog_packages BEGIN
    INSERT INTO catalog_packages_fts(catalog_packages_fts, rowid, name, description)
    VALUES ('delete', old.rowid, old.name, COALESCE(old.description, ''));
END;

CREATE TRIGGER IF NOT EXISTS catalog_packages_au AFTER UPDATE ON catalog_packages BEGIN
    INSERT INTO catalog_packages_fts(catalog_packages_fts, rowid, name, description)
    VALUES ('delete', old.rowid, old.name, COALESCE(old.description, ''));
    INSERT INTO catalog_packages_fts(rowid, name, description)
    VALUES (new.rowid, new.name, COALESCE(new.description, ''));
END;

CREATE TRIGGER IF NOT EXISTS catalog_packages_update_timestamp
AFTER UPDATE ON catalog_packages
FOR EACH ROW
WHEN NEW.updated_at = OLD.updated_at
BEGIN
    UPDATE catalog_packages
    SET updated_at = datetime('now')
    WHERE rowid = NEW.rowid;
END;
