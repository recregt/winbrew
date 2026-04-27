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
	if err := os.MkdirAll(manifestDir, 0o750); err != nil {
		t.Fatalf("MkdirAll() error = %v", err)
	}

	manifest := map[string]any{
		"version":     "1.2.3",
		"description": "example package",
		"homepage":    "https://example.invalid",
		"architecture": map[string]any{
			"64bit": map[string]any{
				"url":  []any{"https://example.invalid/64bit.zip"},
				"hash": []any{"hash-64bit"},
			},
			"32bit": map[string]any{
				"url":  []any{"https://example.invalid/32bit.zip"},
				"hash": []any{"hash-32bit"},
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
		URL:  "https://example.invalid/64bit.zip",
		Hash: "hash-64bit",
		Arch: "x64",
		Type: "portable",
	}, {
		URL:  "https://example.invalid/32bit.zip",
		Hash: "hash-32bit",
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
			"32bit": {URL: []any{"https://example.invalid/32bit.zip"}, Hash: []any{"hash-32bit"}},
			"64bit": {URL: []any{"https://example.invalid/64bit.zip"}, Hash: []any{"hash-64bit"}},
			"any":   {URL: []any{"https://example.invalid/any.zip"}, Hash: []any{"hash-any"}},
			"arm64": {URL: []any{"https://example.invalid/arm64.zip"}, Hash: []any{"hash-arm64"}},
		},
	})

	want := []normalize.Installer{{URL: "https://example.invalid/64bit.zip", Hash: "hash-64bit", Arch: "x64", Type: "portable"}, {URL: "https://example.invalid/32bit.zip", Hash: "hash-32bit", Arch: "x86", Type: "portable"}, {URL: "https://example.invalid/arm64.zip", Hash: "hash-arm64", Arch: "arm64", Type: "portable"}, {URL: "https://example.invalid/any.zip", Hash: "hash-any", Arch: "any", Type: "portable"}}
	if !reflect.DeepEqual(installers, want) {
		t.Fatalf("installers = %#v, want %#v", installers, want)
	}
}

func TestResolveInstallersNormalizesArchitectureAliases(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name     string
		key      string
		wantArch string
	}{
		{name: "64bit", key: "64bit", wantArch: "x64"},
		{name: "x64", key: "x64", wantArch: "x64"},
		{name: "amd64", key: "amd64", wantArch: "x64"},
		{name: "32bit", key: "32bit", wantArch: "x86"},
		{name: "x86", key: "x86", wantArch: "x86"},
		{name: "386", key: "386", wantArch: "x86"},
		{name: "arm64", key: "arm64", wantArch: "arm64"},
		{name: "aarch64", key: "aarch64", wantArch: "arm64"},
		{name: "any", key: "any", wantArch: "any"},
		{name: "neutral", key: "neutral", wantArch: "any"},
	}

	for _, tt := range tests {
		tt := tt
		t.Run(tt.name, func(t *testing.T) {
			t.Parallel()

			installers := resolveInstallers(scoopManifest{
				Architecture: map[string]archBlock{
					tt.key: {
						URL:  []any{"https://example.invalid/installer.zip"},
						Hash: []any{"hash"},
					},
				},
			})

			if len(installers) != 1 {
				t.Fatalf("len(installers) = %d, want 1", len(installers))
			}
			if got, want := installers[0].Arch, tt.wantArch; got != want {
				t.Fatalf("installer arch = %q, want %q", got, want)
			}
		})
	}
}

func TestScoopEnvelopeFromPackage(t *testing.T) {
	t.Parallel()

	envelope := scoopEnvelopeFromPackage(normalize.Package{
		ID:      "scoop/main/example",
		Name:    "example",
		Version: "1.2.3",
	})

	if got, want := envelope.SchemaVersion, scoopEnvelopeSchemaVersion; got != want {
		t.Fatalf("SchemaVersion = %d, want %d", got, want)
	}
	if got, want := envelope.Source, sourceName; got != want {
		t.Fatalf("Source = %q, want %q", got, want)
	}
	if got, want := envelope.Kind, scoopEnvelopeKind; got != want {
		t.Fatalf("Kind = %q, want %q", got, want)
	}
	if got, want := envelope.Payload.ID, "scoop/main/example"; got != want {
		t.Fatalf("Payload.ID = %q, want %q", got, want)
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
