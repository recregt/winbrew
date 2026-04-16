package publisher

import (
	"crypto/sha256"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

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

	if strings.Contains(sql, "'patch'") {
		t.Fatalf("sql = %q, want full fallback rather than patch row", sql)
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
