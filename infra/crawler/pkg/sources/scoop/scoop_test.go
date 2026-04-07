package scoop

import (
	"context"
	"encoding/json"
	"io"
	"os"
	"path/filepath"
	"reflect"
	"testing"

	"infra/crawler/pkg/normalize"
)

func TestNewDeduplicatesBuckets(t *testing.T) {
	t.Parallel()

	dir := t.TempDir()
	source, err := New(dir,
		Bucket{Name: "extras", URL: "https://example.invalid/override"},
		Bucket{Name: "custom", URL: "https://example.invalid/custom"},
	)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	got := make([]string, 0, len(source.buckets))
	for _, bucket := range source.buckets {
		got = append(got, bucket.Name)
	}

	want := []string{"main", "extras", "versions", "games", "custom"}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("bucket names = %#v, want %#v", got, want)
	}
}

func TestReadManifestUsesArchitectureBlocks(t *testing.T) {
	t.Parallel()

	dir := t.TempDir()
	manifestDir := filepath.Join(dir, "bucket")
	if err := os.MkdirAll(manifestDir, 0o755); err != nil {
		t.Fatalf("MkdirAll() error = %v", err)
	}

	manifest := map[string]any{
		"version":     "1.2.3",
		"description": "example package",
		"homepage":    "https://example.invalid",
		"architecture": map[string]any{
			"x64": map[string]any{
				"url":  []any{"https://example.invalid/x64.zip"},
				"hash": []any{"hash-x64"},
			},
			"x86": map[string]any{
				"url":  []any{"https://example.invalid/x86.zip"},
				"hash": []any{"hash-x86"},
			},
		},
	}
	data, err := json.Marshal(manifest)
	if err != nil {
		t.Fatalf("json.Marshal() error = %v", err)
	}
	if err := os.WriteFile(filepath.Join(manifestDir, "example.json"), data, 0o644); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}

	pkg, err := readManifest(context.Background(), "main", manifestDir, "example.json")
	if err != nil {
		t.Fatalf("readManifest() error = %v", err)
	}

	want := []normalize.Installer{{
		URL:  "https://example.invalid/x64.zip",
		Hash: "hash-x64",
		Arch: "x64",
		Type: "portable",
	}, {
		URL:  "https://example.invalid/x86.zip",
		Hash: "hash-x86",
		Arch: "x86",
		Type: "portable",
	}}
	if len(pkg.Installers) != len(want) {
		t.Fatalf("len(Installers) = %d, want %d", len(pkg.Installers), len(want))
	}
	for i := range want {
		if pkg.Installers[i] != want[i] {
			t.Fatalf("Installers[%d] = %#v, want %#v", i, pkg.Installers[i], want[i])
		}
	}
}

func TestResolveLicenseUsesIdentifierOrURL(t *testing.T) {
	t.Parallel()

	if got := resolveLicense(map[string]any{"identifier": "MIT"}); got != "MIT" {
		t.Fatalf("resolveLicense(identifier) = %q, want %q", got, "MIT")
	}
	if got := resolveLicense(map[string]any{"url": "https://example.invalid/license"}); got != "https://example.invalid/license" {
		t.Fatalf("resolveLicense(url) = %q, want %q", got, "https://example.invalid/license")
	}
	if got := resolveLicense(nil); got != "" {
		t.Fatalf("resolveLicense(nil) = %q, want empty", got)
	}
}

func TestResolveInstallersUsesArchitectureOrder(t *testing.T) {
	t.Parallel()

	installers := resolveInstallers(scoopManifest{
		Architecture: map[string]archBlock{
			"amd64": {URL: []any{"https://example.invalid/amd64.zip"}, Hash: []any{"hash-amd64"}},
			"any":   {URL: []any{"https://example.invalid/any.zip"}, Hash: []any{"hash-any"}},
			"x64":   {URL: []any{"https://example.invalid/x64.zip"}, Hash: []any{"hash-x64"}},
		},
	})

	want := []normalize.Installer{{URL: "https://example.invalid/x64.zip", Hash: "hash-x64", Arch: "x64", Type: "portable"}, {URL: "https://example.invalid/amd64.zip", Hash: "hash-amd64", Arch: "amd64", Type: "portable"}, {URL: "https://example.invalid/any.zip", Hash: "hash-any", Arch: "any", Type: "portable"}}
	if !reflect.DeepEqual(installers, want) {
		t.Fatalf("installers = %#v, want %#v", installers, want)
	}
}

func TestWriteBucketJSONLMissingManifestDir(t *testing.T) {
	t.Parallel()

	dir := t.TempDir()
	err := writeBucketJSONL(context.Background(), json.NewEncoder(io.Discard), "main", filepath.Join(dir, "missing-bucket"))
	if err != nil {
		t.Fatalf("writeBucketJSONL() error = %v", err)
	}
}
