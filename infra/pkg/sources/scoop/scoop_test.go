package scoop

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"winbrew/infra/pkg/normalize"
)

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

	pkg, err := readManifest("main", manifestDir, "example.json")
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

func TestReadBucketMissingManifestDir(t *testing.T) {
	t.Parallel()

	dir := t.TempDir()
	pkgs, err := readBucket(context.Background(), "main", filepath.Join(dir, "missing-bucket"))
	if err != nil {
		t.Fatalf("readBucket() error = %v", err)
	}
	if pkgs != nil {
		t.Fatalf("readBucket() pkgs = %#v, want nil", pkgs)
	}
}
