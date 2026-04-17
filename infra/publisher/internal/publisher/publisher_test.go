package publisher

import (
	"bytes"
	"crypto/sha256"
	"database/sql"
	"fmt"
	"io"
	"net/url"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"testing"

	"github.com/klauspost/compress/zstd"
	"github.com/minio/minio-go/v7"
)

func TestNormalizeEndpoint(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name       string
		input      string
		wantHost   string
		wantSecure bool
		wantErr    bool
	}{
		{name: "https", input: "https://123.r2.cloudflarestorage.com", wantHost: "123.r2.cloudflarestorage.com", wantSecure: true},
		{name: "http", input: "http://localhost:9000", wantHost: "localhost:9000", wantSecure: false},
		{name: "bare host port", input: "localhost:9000", wantHost: "localhost:9000", wantSecure: true},
		{name: "path not allowed", input: "https://123.r2.cloudflarestorage.com/path", wantErr: true},
	}

	for _, testCase := range tests {
		testCase := testCase
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			host, secure, err := normalizeEndpoint(testCase.input)
			if testCase.wantErr {
				if err == nil {
					t.Fatal("normalizeEndpoint() error = nil, want error")
				}
				return
			}
			if err != nil {
				t.Fatalf("normalizeEndpoint() error = %v", err)
			}
			if got, want := host, testCase.wantHost; got != want {
				t.Fatalf("host = %q, want %q", got, want)
			}
			if secure != testCase.wantSecure {
				t.Fatalf("secure = %v, want %v", secure, testCase.wantSecure)
			}
		})
	}
}

func TestIsMissingObject(t *testing.T) {
	t.Parallel()

	if !isMissingObject(minio.ErrorResponse{Code: "NoSuchKey", StatusCode: 404}) {
		t.Fatal("isMissingObject(NoSuchKey) = false, want true")
	}
	if !isMissingObject(minio.ErrorResponse{Code: "NoSuchObject", StatusCode: 404}) {
		t.Fatal("isMissingObject(NoSuchObject) = false, want true")
	}
	if !isMissingObject(minio.ErrorResponse{StatusCode: 404}) {
		t.Fatal("isMissingObject(404) = false, want true")
	}
	if isMissingObject(minio.ErrorResponse{Code: "AccessDenied", StatusCode: 403}) {
		t.Fatal("isMissingObject(AccessDenied) = true, want false")
	}
}

func TestFirstNonEmpty(t *testing.T) {
	t.Parallel()

	if got, want := firstNonEmpty("", "  ", "value", "later"), "value"; got != want {
		t.Fatalf("firstNonEmpty() = %q, want %q", got, want)
	}
}

func TestMetadataKeyForObjectKey(t *testing.T) {
	t.Parallel()

	if got, want := metadataKeyForObjectKey("catalog.db"), "metadata.json"; got != want {
		t.Fatalf("metadataKeyForObjectKey() = %q, want %q", got, want)
	}

	if got, want := metadataKeyForObjectKey("release/latest/catalog.db"), "release/latest/metadata.json"; got != want {
		t.Fatalf("metadataKeyForObjectKey() = %q, want %q", got, want)
	}
}

func TestMetadataTempKeyForObjectKey(t *testing.T) {
	t.Parallel()

	if got, want := metadataTempKeyForObjectKey("catalog.db"), "metadata.json.tmp"; got != want {
		t.Fatalf("metadataTempKeyForObjectKey() = %q, want %q", got, want)
	}

	if got, want := metadataTempKeyForObjectKey("release/latest/catalog.db"), "release/latest/metadata.json.tmp"; got != want {
		t.Fatalf("metadataTempKeyForObjectKey() = %q, want %q", got, want)
	}
}

func TestSQLiteDSNPrefixesWindowsDrivePath(t *testing.T) {
	t.Parallel()

	dbPath := filepath.Join(t.TempDir(), "catalog.db")
	if runtime.GOOS == "windows" {
		dbPath = `C:\Users\recregt\AppData\Local\winbrew\catalog.db`
	}

	dsn, err := sqliteDSN(dbPath)
	if err != nil {
		t.Fatalf("sqliteDSN() error = %v", err)
	}

	absPath, err := filepath.Abs(dbPath)
	if err != nil {
		t.Fatalf("filepath.Abs() error = %v", err)
	}
	wantPath := filepath.ToSlash(absPath)
	if runtime.GOOS == "windows" && len(wantPath) >= 2 && wantPath[1] == ':' {
		wantPath = "/" + wantPath
	}

	if got, want := dsn, (&url.URL{Scheme: "file", Path: wantPath, RawQuery: "mode=ro"}).String(); got != want {
		t.Fatalf("sqliteDSN() = %q, want %q", got, want)
	}
}

func TestObjectTempKeyForObjectKey(t *testing.T) {
	t.Parallel()

	if got, want := objectTempKeyForObjectKey("catalog.db"), "catalog.db.tmp"; got != want {
		t.Fatalf("objectTempKeyForObjectKey() = %q, want %q", got, want)
	}

	if got, want := objectTempKeyForObjectKey("release/latest/catalog.db"), "release/latest/catalog.db.tmp"; got != want {
		t.Fatalf("objectTempKeyForObjectKey() = %q, want %q", got, want)
	}
}

func TestHashFileAndMetadataRoundTrip(t *testing.T) {
	t.Parallel()

	dir := t.TempDir()
	path := filepath.Join(dir, "catalog.db")
	if err := os.WriteFile(path, []byte("catalog-bytes"), 0o644); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}

	hash, err := hashFile(path)
	if err != nil {
		t.Fatalf("hashFile() error = %v", err)
	}
	expected := "sha256:" + fmt.Sprintf("%x", sha256.Sum256([]byte("catalog-bytes")))
	if got, want := hash, expected; got != want {
		t.Fatalf("hashFile() = %q, want %q", got, want)
	}

	metadata := Metadata{
		SchemaVersion:   1,
		GeneratedAtUnix: 1,
		CurrentHash:     hash,
		PackageCount:    1,
		SourceCounts:    map[string]int{"scoop": 1},
	}

	metadataPath := filepath.Join(dir, "metadata.json")
	if err := SaveMetadata(metadataPath, metadata); err != nil {
		t.Fatalf("SaveMetadata() error = %v", err)
	}

	restored, err := LoadMetadata(metadataPath)
	if err != nil {
		t.Fatalf("LoadMetadata() error = %v", err)
	}
	if got, want := restored.CurrentHash, metadata.CurrentHash; got != want {
		t.Fatalf("CurrentHash = %q, want %q", got, want)
	}
	if got, want := restored.PackageCount, metadata.PackageCount; got != want {
		t.Fatalf("PackageCount = %d, want %d", got, want)
	}
}

func TestCompressSnapshotToTempRoundTrips(t *testing.T) {
	t.Parallel()

	dir := t.TempDir()
	inputPath := filepath.Join(dir, "catalog.db")
	rawBytes := bytes.Repeat([]byte("winbrew-catalog-snapshot-"), 128)
	if err := os.WriteFile(inputPath, rawBytes, 0o644); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}
	compressedPath, compressedSize, err := compressSnapshotToTemp(inputPath)
	if err != nil {
		t.Fatalf("compressSnapshotToTemp() error = %v", err)
	}
	defer func() {
		_ = os.Remove(compressedPath)
	}()

	if compressedSize <= 0 {
		t.Fatalf("compressed size = %d, want > 0", compressedSize)
	}

	compressedBytes, err := os.ReadFile(compressedPath)
	if err != nil {
		t.Fatalf("ReadFile() error = %v", err)
	}

	decoder, err := zstd.NewReader(bytes.NewReader(compressedBytes))
	if err != nil {
		t.Fatalf("zstd.NewReader() error = %v", err)
	}
	defer decoder.Close()

	decompressedBytes, err := io.ReadAll(decoder)
	if err != nil {
		t.Fatalf("ReadAll() error = %v", err)
	}

	if !bytes.Equal(decompressedBytes, rawBytes) {
		t.Fatalf("decompressed snapshot mismatch")
	}
}

func TestBuildCatalogPatchSQLReproducesCurrentSnapshot(t *testing.T) {
	t.Parallel()

	schemaStatements := []string{
		`CREATE TABLE IF NOT EXISTS catalog_packages (id TEXT PRIMARY KEY, name TEXT NOT NULL, version TEXT NOT NULL, source TEXT NOT NULL, namespace TEXT, source_id TEXT NOT NULL, description TEXT, homepage TEXT, license TEXT, publisher TEXT, locale TEXT, moniker TEXT, tags TEXT, bin TEXT, created_at TEXT NOT NULL, updated_at TEXT NOT NULL);`,
		`CREATE TABLE IF NOT EXISTS catalog_installers (id INTEGER PRIMARY KEY AUTOINCREMENT, package_id TEXT NOT NULL, url TEXT NOT NULL, hash TEXT, hash_algorithm TEXT NOT NULL, installer_type TEXT NOT NULL, installer_switches TEXT, scope TEXT, arch TEXT NOT NULL, kind TEXT NOT NULL, nested_kind TEXT);`,
		`CREATE TABLE IF NOT EXISTS catalog_packages_raw (package_id TEXT PRIMARY KEY, raw TEXT);`,
		`CREATE TRIGGER IF NOT EXISTS catalog_packages_delete_cleanup AFTER DELETE ON catalog_packages BEGIN DELETE FROM catalog_packages_raw WHERE package_id = old.id; DELETE FROM catalog_installers WHERE package_id = old.id; END;`,
	}

	previousStatements := []string{
		`INSERT INTO catalog_packages (rowid, id, name, version, source, namespace, source_id, description, homepage, license, publisher, locale, moniker, tags, bin, created_at, updated_at) VALUES (1, 'pkg/a', 'Alpha', '1.0.0', 'winget', NULL, 'pkg.a', 'Alpha desc', NULL, NULL, 'Alpha Inc.', NULL, NULL, NULL, NULL, '2026-04-15 10:00:00', '2026-04-15 10:00:00');`,
		`INSERT INTO catalog_packages_raw (package_id, raw) VALUES ('pkg/a', '{"id":"pkg/a"}');`,
		`INSERT INTO catalog_installers (id, package_id, url, hash, hash_algorithm, installer_type, installer_switches, scope, arch, kind, nested_kind) VALUES (1, 'pkg/a', 'https://example.invalid/a.exe', 'sha256:old', 'sha256', 'exe', NULL, NULL, 'x64', 'exe', NULL);`,
		`INSERT INTO catalog_packages (rowid, id, name, version, source, namespace, source_id, description, homepage, license, publisher, locale, moniker, tags, bin, created_at, updated_at) VALUES (2, 'pkg/b', 'Beta', '1.0.0', 'winget', NULL, 'pkg.b', 'Beta desc', NULL, NULL, 'Beta Inc.', NULL, NULL, NULL, NULL, '2026-04-15 10:00:00', '2026-04-15 10:00:00');`,
		`INSERT INTO catalog_packages_raw (package_id, raw) VALUES ('pkg/b', '{"id":"pkg/b"}');`,
		`INSERT INTO catalog_installers (id, package_id, url, hash, hash_algorithm, installer_type, installer_switches, scope, arch, kind, nested_kind) VALUES (2, 'pkg/b', 'https://example.invalid/b.exe', 'sha256:b', 'sha256', 'exe', NULL, NULL, 'x64', 'exe', NULL);`,
	}
	currentStatements := []string{
		`INSERT INTO catalog_packages (rowid, id, name, version, source, namespace, source_id, description, homepage, license, publisher, locale, moniker, tags, bin, created_at, updated_at) VALUES (1, 'pkg/a', 'Alpha', '1.1.0', 'winget', NULL, 'pkg.a', 'Alpha desc', NULL, NULL, 'Alpha Inc.', NULL, NULL, NULL, NULL, '2026-04-15 10:00:00', '2026-04-16 12:00:00');`,
		`INSERT INTO catalog_packages_raw (package_id, raw) VALUES ('pkg/a', '{"id":"pkg/a","updated":true}');`,
		`INSERT INTO catalog_installers (id, package_id, url, hash, hash_algorithm, installer_type, installer_switches, scope, arch, kind, nested_kind) VALUES (1, 'pkg/a', 'https://example.invalid/a.exe', 'sha256:new', 'sha256', 'exe', NULL, NULL, 'x64', 'exe', NULL);`,
		`INSERT INTO catalog_packages (rowid, id, name, version, source, namespace, source_id, description, homepage, license, publisher, locale, moniker, tags, bin, created_at, updated_at) VALUES (3, 'pkg/c', 'Gamma', '1.0.0', 'winget', NULL, 'pkg.c', 'Gamma desc', NULL, NULL, 'Gamma Inc.', NULL, NULL, NULL, NULL, '2026-04-16 12:00:00', '2026-04-16 12:00:00');`,
		`INSERT INTO catalog_packages_raw (package_id, raw) VALUES ('pkg/c', '{"id":"pkg/c"}');`,
		`INSERT INTO catalog_installers (id, package_id, url, hash, hash_algorithm, installer_type, installer_switches, scope, arch, kind, nested_kind) VALUES (3, 'pkg/c', 'https://example.invalid/c.exe', 'sha256:c', 'sha256', 'exe', NULL, NULL, 'x64', 'exe', NULL);`,
	}

	openSnapshotDB := func(name string) (*sql.DB, error) {
		dsn := fmt.Sprintf("file:%s?mode=memory&cache=shared", name)
		return sql.Open("sqlite", dsn)
	}

	buildSnapshot := func(name string, statements []string) *sql.DB {
		t.Helper()

		db, err := openSnapshotDB(name)
		if err != nil {
			t.Fatalf("open snapshot db: %v", err)
		}

		for _, statement := range schemaStatements {
			if _, err := db.Exec(statement); err != nil {
				t.Fatalf("exec schema statement %q: %v", statement, err)
			}
		}
		for _, statement := range statements {
			if _, err := db.Exec(statement); err != nil {
				t.Fatalf("exec snapshot statement %q: %v", statement, err)
			}
		}

		return db
	}

	previousDB := buildSnapshot("publisher_previous", previousStatements)
	defer previousDB.Close()
	currentDB := buildSnapshot("publisher_current", currentStatements)
	defer currentDB.Close()

	patchSQL, err := buildCatalogPatchSQLFromDB(previousDB, currentDB)
	if err != nil {
		t.Fatalf("buildCatalogPatchSQL() error = %v", err)
	}

	if _, err := previousDB.Exec(patchSQL); err != nil {
		t.Fatalf("exec patch sql: %v", err)
	}

	gotPackages, gotRaws, gotInstallers, err := loadCatalogSnapshot(previousDB)
	if err != nil {
		t.Fatalf("load patched snapshot: %v", err)
	}
	wantPackages, wantRaws, wantInstallers, err := loadCatalogSnapshot(currentDB)
	if err != nil {
		t.Fatalf("load current snapshot: %v", err)
	}

	if len(gotPackages) != len(wantPackages) {
		t.Fatalf("package count = %d, want %d", len(gotPackages), len(wantPackages))
	}
	for id, wantPackage := range wantPackages {
		gotPackage, ok := gotPackages[id]
		if !ok {
			t.Fatalf("missing patched package %q", id)
		}
		if !packageRecordsEqual(gotPackage, wantPackage) {
			t.Fatalf("patched package %q mismatch", id)
		}
	}

	if len(gotRaws) != len(wantRaws) {
		t.Fatalf("raw row count = %d, want %d", len(gotRaws), len(wantRaws))
	}
	for packageID, wantRaw := range wantRaws {
		gotRaw, ok := gotRaws[packageID]
		if !ok {
			t.Fatalf("missing patched raw row %q", packageID)
		}
		if !nullStringEqual(gotRaw, wantRaw) {
			t.Fatalf("patched raw row %q mismatch", packageID)
		}
	}

	if len(gotInstallers) != len(wantInstallers) {
		t.Fatalf("installer package count = %d, want %d", len(gotInstallers), len(wantInstallers))
	}
	for packageID, wantInstallersForPackage := range wantInstallers {
		gotInstallersForPackage, ok := gotInstallers[packageID]
		if !ok {
			t.Fatalf("missing patched installers for %q", packageID)
		}
		if len(gotInstallersForPackage) != len(wantInstallersForPackage) {
			t.Fatalf("installer count for %q = %d, want %d", packageID, len(gotInstallersForPackage), len(wantInstallersForPackage))
		}
		for installerID, wantInstaller := range wantInstallersForPackage {
			gotInstaller, ok := gotInstallersForPackage[installerID]
			if !ok {
				t.Fatalf("missing patched installer %d for %q", installerID, packageID)
			}
			if !installerRecordsEqual(gotInstaller, wantInstaller) {
				t.Fatalf("patched installer %d for %q mismatch", installerID, packageID)
			}
		}
	}
}

func TestLoadMetadataRejectsUnsupportedSchemaVersion(t *testing.T) {
	t.Parallel()

	dir := t.TempDir()
	path := filepath.Join(dir, "metadata.json")
	if err := os.WriteFile(path, []byte(`{"schema_version":2,"generated_at_unix":1,"current_hash":"sha256:abc","package_count":1,"source_counts":{}}`), 0o644); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}

	if _, err := LoadMetadata(path); err == nil {
		t.Fatal("LoadMetadata() error = nil, want unsupported schema version")
	}
}

func TestBuildUpdatePlansSQLIncludesCurrentAndFullRows(t *testing.T) {
	metadata := Metadata{
		SchemaVersion:   1,
		GeneratedAtUnix: 1,
		CurrentHash:     "sha256:new",
		PreviousHash:    "sha256:old",
		PackageCount:    1,
		SourceCounts:    map[string]int{"scoop": 1},
	}

	sql, err := buildUpdatePlansSQL(
		"https://cdn.example.invalid/base",
		"catalog/latest.db.zst",
		metadata,
		1024,
		nil,
	)
	if err != nil {
		t.Fatalf("buildUpdatePlansSQL() error = %v", err)
	}

	if got, want := strings.Count(sql, "INSERT INTO update_plans"), 2; got != want {
		t.Fatalf("insert count = %d, want %d", got, want)
	}

	if strings.Contains(sql, "BEGIN;") || strings.Contains(sql, "COMMIT;") {
		t.Fatalf("sql = %q, want no explicit transaction statements", sql)
	}

	if !strings.Contains(sql, "CREATE TABLE IF NOT EXISTS schema_meta") {
		t.Fatalf("sql = %q, want schema bootstrap to be present", sql)
	}

	if !strings.Contains(sql, "CREATE TABLE IF NOT EXISTS update_plans") {
		t.Fatalf("sql = %q, want update_plans bootstrap to be present", sql)
	}

	if !strings.Contains(sql, "https://cdn.example.invalid/base/catalog/latest.db.zst") {
		t.Fatalf("sql = %q, want snapshot URL to be present", sql)
	}

	if !strings.Contains(sql, "VALUES ('sha256:old', 'full', 'sha256:new', 'https://cdn.example.invalid/base/catalog/latest.db.zst', '[]', 0, 0, 1, 0);") {
		t.Fatalf("sql = %q, want full row to be present", sql)
	}

	if !strings.Contains(sql, "VALUES ('sha256:new', 'current', 'sha256:new', NULL, '[]', 0, 0, 0, 0);") {
		t.Fatalf("sql = %q, want current row to be present", sql)
	}
}

func TestBuildUpdatePlansSQLUsesSingleFullRowWithoutPreviousHash(t *testing.T) {
	metadata := Metadata{
		SchemaVersion:   1,
		GeneratedAtUnix: 1,
		CurrentHash:     "sha256:new",
		PackageCount:    1,
		SourceCounts:    map[string]int{"scoop": 1},
	}

	sql, err := buildUpdatePlansSQL(
		"https://cdn.example.invalid/base",
		"catalog/latest.db.zst",
		metadata,
		1024,
		nil,
	)
	if err != nil {
		t.Fatalf("buildUpdatePlansSQL() error = %v", err)
	}

	if got, want := strings.Count(sql, "INSERT INTO update_plans"), 1; got != want {
		t.Fatalf("insert count = %d, want %d", got, want)
	}

	if strings.Contains(sql, "BEGIN;") || strings.Contains(sql, "COMMIT;") {
		t.Fatalf("sql = %q, want no explicit transaction statements", sql)
	}

	if !strings.Contains(sql, "VALUES ('sha256:new', 'full', 'sha256:new', 'https://cdn.example.invalid/base/catalog/latest.db.zst', '[]', 0, 0, 1, 0);") {
		t.Fatalf("sql = %q, want single full row to be present", sql)
	}
}

func TestBuildUpdatePlansSQLUsesPatchChainWhenAvailable(t *testing.T) {
	metadata := Metadata{
		SchemaVersion:   1,
		GeneratedAtUnix: 1,
		CurrentHash:     "sha256:new",
		PreviousHash:    "sha256:old",
		PackageCount:    1,
		SourceCounts:    map[string]int{"scoop": 1},
	}

	sql, err := buildUpdatePlansSQL(
		"https://cdn.example.invalid/base",
		"catalog/latest.db.zst",
		metadata,
		3000,
		[]patchChainArtifact{
			{Depth: 1, FilePath: "patches/001.sql.zst", SizeBytes: 500, ReachedPrevious: true},
			{Depth: 0, FilePath: "patches/002.sql.zst", SizeBytes: 400, ReachedPrevious: true},
		},
	)
	if err != nil {
		t.Fatalf("buildUpdatePlansSQL() error = %v", err)
	}

	if got, want := strings.Count(sql, "INSERT INTO update_plans"), 3; got != want {
		t.Fatalf("insert count = %d, want %d", got, want)
	}

	if strings.Contains(sql, "BEGIN;") || strings.Contains(sql, "COMMIT;") {
		t.Fatalf("sql = %q, want no explicit transaction statements", sql)
	}

	if !strings.Contains(sql, "VALUES ('full:sha256:new', 'full', 'sha256:new', 'https://cdn.example.invalid/base/catalog/latest.db.zst', '[]', 0, 0, 1, 0);") {
		t.Fatalf("sql = %q, want synthetic latest full row to be present", sql)
	}

	if !strings.Contains(sql, "VALUES ('sha256:old', 'patch', 'sha256:new', NULL, '[\"https://cdn.example.invalid/base/patches/001.sql.zst\",\"https://cdn.example.invalid/base/patches/002.sql.zst\"]', 2, 900, 0, 0);") {
		t.Fatalf("sql = %q, want patch row to be present", sql)
	}

	if !strings.Contains(sql, "VALUES ('sha256:new', 'current', 'sha256:new', NULL, '[]', 0, 0, 0, 0);") {
		t.Fatalf("sql = %q, want current row to be present", sql)
	}
}

func TestBuildUpdatePlansSQLFallsBackToFullWhenPatchChainIsTooLarge(t *testing.T) {
	metadata := Metadata{
		SchemaVersion:   1,
		GeneratedAtUnix: 1,
		CurrentHash:     "sha256:new",
		PreviousHash:    "sha256:old",
		PackageCount:    1,
		SourceCounts:    map[string]int{"scoop": 1},
	}

	sql, err := buildUpdatePlansSQL(
		"https://cdn.example.invalid/base",
		"catalog/latest.db.zst",
		metadata,
		1000,
		[]patchChainArtifact{{Depth: 0, FilePath: "patches/001.sql.zst", SizeBytes: 500, ReachedPrevious: true}},
	)
	if err != nil {
		t.Fatalf("buildUpdatePlansSQL() error = %v", err)
	}

	if got, want := strings.Count(sql, "INSERT INTO update_plans"), 2; got != want {
		t.Fatalf("insert count = %d, want %d", got, want)
	}

	if strings.Contains(sql, "BEGIN;") || strings.Contains(sql, "COMMIT;") {
		t.Fatalf("sql = %q, want no explicit transaction statements", sql)
	}

	if strings.Contains(sql, "VALUES ('sha256:old', 'patch', 'sha256:new'") {
		t.Fatalf("sql = %q, want full fallback rather than patch row", sql)
	}
}

func TestBuildReleaseMaterializationSQLDoesNotWrapTransaction(t *testing.T) {
	t.Parallel()

	metadata := Metadata{
		SchemaVersion:   1,
		GeneratedAtUnix: 1,
		CurrentHash:     "sha256:new",
		PreviousHash:    "sha256:old",
		PackageCount:    1,
		SourceCounts:    map[string]int{"scoop": 1},
	}

	sql, err := buildReleaseMaterializationSQL(
		"https://cdn.example.invalid/base",
		"catalog/latest.db.zst",
		metadata,
		[]patchChainArtifact{{FromHash: "sha256:old", ToHash: "sha256:new", FilePath: "patches/001.sql.zst", SizeBytes: 500, Checksum: "sha256:patch", ReachedPrevious: true}},
	)
	if err != nil {
		t.Fatalf("buildReleaseMaterializationSQL() error = %v", err)
	}

	if strings.Contains(sql, "BEGIN;") || strings.Contains(sql, "COMMIT;") {
		t.Fatalf("sql = %q, want no explicit transaction statements", sql)
	}

	if !strings.Contains(sql, "CREATE TABLE IF NOT EXISTS release_lineage") {
		t.Fatalf("sql = %q, want release_lineage bootstrap to be present", sql)
	}

	if !strings.Contains(sql, "CREATE TABLE IF NOT EXISTS patch_artifacts") {
		t.Fatalf("sql = %q, want patch_artifacts bootstrap to be present", sql)
	}

	if !strings.Contains(sql, "CREATE TABLE IF NOT EXISTS update_plans") {
		t.Fatalf("sql = %q, want update_plans bootstrap to be present", sql)
	}

	if !strings.Contains(sql, "INSERT INTO release_lineage") {
		t.Fatalf("sql = %q, want release lineage insert to be present", sql)
	}
}

func TestGeneratedD1SQLBootstrapsEmptyDatabase(t *testing.T) {
	releaseMetadata := Metadata{
		SchemaVersion:   1,
		GeneratedAtUnix: 1,
		CurrentHash:     "sha256:new",
		PreviousHash:    "sha256:old",
		PackageCount:    1,
		SourceCounts:    map[string]int{"scoop": 1},
	}

	releaseSQL, err := buildReleaseMaterializationSQL(
		"https://cdn.example.invalid/base",
		"catalog/latest.db.zst",
		releaseMetadata,
		[]patchChainArtifact{{FromHash: "sha256:old", ToHash: "sha256:new", FilePath: "patches/001.sql.zst", SizeBytes: 500, Checksum: "sha256:patch", ReachedPrevious: true}},
	)
	if err != nil {
		t.Fatalf("buildReleaseMaterializationSQL() error = %v", err)
	}

	updateSQL, err := buildUpdatePlansSQL(
		"https://cdn.example.invalid/base",
		"catalog/latest.db.zst",
		releaseMetadata,
		1024,
		nil,
	)
	if err != nil {
		t.Fatalf("buildUpdatePlansSQL() error = %v", err)
	}

	db := openTestPublisherDB(t)
	defer db.Close()

	execSQLScript(t, db, releaseSQL)
	execSQLScript(t, db, updateSQL)
}

func openTestPublisherDB(t *testing.T) *sql.DB {
	t.Helper()

	db, err := sql.Open("sqlite", filepath.Join(t.TempDir(), "publisher-bootstrap.db"))
	if err != nil {
		t.Fatalf("sql.Open() error = %v", err)
	}

	return db
}

func execSQLScript(t *testing.T, db *sql.DB, script string) {
	t.Helper()

	for _, statement := range strings.Split(script, "\n") {
		statement = strings.TrimSpace(statement)
		if statement == "" {
			continue
		}

		if _, err := db.Exec(statement); err != nil {
			t.Fatalf("db.Exec(%q) error = %v", statement, err)
		}
	}
}

func TestPublicObjectURLUsesRootBaseURL(t *testing.T) {
	url, err := publicObjectURL("https://cdn.example.invalid", "catalog/latest.db.zst")
	if err != nil {
		t.Fatalf("publicObjectURL() error = %v", err)
	}

	if got, want := url, "https://cdn.example.invalid/catalog/latest.db.zst"; got != want {
		t.Fatalf("publicObjectURL() = %q, want %q", got, want)
	}
}
