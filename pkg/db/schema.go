package db

const schema = `
CREATE TABLE IF NOT EXISTS packages (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    version     TEXT NOT NULL,
    source      TEXT NOT NULL,
    description TEXT,
    homepage    TEXT,
    license     TEXT,
    publisher   TEXT,
    raw         TEXT CHECK (raw IS NULL OR json_valid(raw))
);

CREATE TABLE IF NOT EXISTS installers (
    id          INTEGER PRIMARY KEY AUTOINCREMENT,
    package_id  TEXT NOT NULL REFERENCES packages(id) ON DELETE CASCADE,
    url         TEXT NOT NULL,
    hash        TEXT NOT NULL,
    arch        TEXT NOT NULL DEFAULT '',
    type        TEXT NOT NULL DEFAULT '',
    UNIQUE(package_id, url, hash, arch, type)
);

CREATE VIRTUAL TABLE IF NOT EXISTS packages_fts USING fts5(
    name,
    description,
    content=packages,
    content_rowid=rowid
);

CREATE INDEX IF NOT EXISTS idx_packages_source  ON packages(source);
CREATE INDEX IF NOT EXISTS idx_packages_name    ON packages(name);
CREATE INDEX IF NOT EXISTS idx_installers_pkg   ON installers(package_id);

CREATE TRIGGER IF NOT EXISTS packages_ai AFTER INSERT ON packages BEGIN
    INSERT INTO packages_fts(rowid, name, description)
    VALUES (new.rowid, new.name, new.description);
END;

CREATE TRIGGER IF NOT EXISTS packages_ad AFTER DELETE ON packages BEGIN
    INSERT INTO packages_fts(packages_fts, rowid, name, description)
    VALUES ('delete', old.rowid, old.name, old.description);
END;

CREATE TRIGGER IF NOT EXISTS packages_au AFTER UPDATE ON packages BEGIN
    INSERT INTO packages_fts(packages_fts, rowid, name, description)
    VALUES ('delete', old.rowid, old.name, old.description);
    INSERT INTO packages_fts(rowid, name, description)
    VALUES (new.rowid, new.name, new.description);
END;
`
