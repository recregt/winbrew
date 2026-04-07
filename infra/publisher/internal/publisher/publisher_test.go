package publisher

import (
	"crypto/sha256"
	"fmt"
	"os"
	"path/filepath"
	"testing"
)

func TestNormalizeEndpoint(t *testing.T) {
	t.Parallel()

	host, secure, err := normalizeEndpoint("https://123.r2.cloudflarestorage.com")
	if err != nil {
		t.Fatalf("normalizeEndpoint() error = %v", err)
	}
	if got, want := host, "123.r2.cloudflarestorage.com"; got != want {
		t.Fatalf("host = %q, want %q", got, want)
	}
	if !secure {
		t.Fatal("secure = false, want true")
	}

	host, secure, err = normalizeEndpoint("http://localhost:9000")
	if err != nil {
		t.Fatalf("normalizeEndpoint() error = %v", err)
	}
	if got, want := host, "localhost:9000"; got != want {
		t.Fatalf("host = %q, want %q", got, want)
	}
	if secure {
		t.Fatal("secure = true, want false")
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
