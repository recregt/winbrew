package scoop

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"strings"

	"winbrew/infra/pkg/normalize"
)

const sourceName = "scoop"

// Official Scoop buckets
var defaultBuckets = []Bucket{
	{Name: "main", URL: "https://github.com/ScoopInstaller/Main"},
	{Name: "extras", URL: "https://github.com/ScoopInstaller/Extras"},
	{Name: "versions", URL: "https://github.com/ScoopInstaller/Versions"},
	{Name: "games", URL: "https://github.com/Calinou/scoop-games"},
}

type Bucket struct {
	Name string
	URL  string
}

type Source struct {
	buckets  []Bucket
	cacheDir string
}

func New(cacheDir string, extra ...Bucket) (*Source, error) {
	if cacheDir == "" {
		return nil, fmt.Errorf("cache dir cannot be empty")
	}
	if err := os.MkdirAll(cacheDir, 0o755); err != nil {
		return nil, fmt.Errorf("failed to create cache dir: %w", err)
	}

	buckets := append([]Bucket{}, defaultBuckets...)
	buckets = append(buckets, extra...)

	return &Source{
		buckets:  buckets,
		cacheDir: cacheDir,
	}, nil
}

func (s *Source) Name() string {
	return sourceName
}

func (s *Source) Fetch(ctx context.Context) ([]normalize.Package, error) {
	var all []normalize.Package

	for _, bucket := range s.buckets {
		if err := ctx.Err(); err != nil {
			return nil, err
		}

		pkgs, err := s.fetchBucket(ctx, bucket)
		if err != nil {
			return nil, fmt.Errorf("bucket %s: %w", bucket.Name, err)
		}
		all = append(all, pkgs...)
	}

	return all, nil
}

func (s *Source) fetchBucket(ctx context.Context, bucket Bucket) ([]normalize.Package, error) {
	bucketDir := filepath.Join(s.cacheDir, bucket.Name)

	if err := syncRepo(ctx, bucket.URL, bucketDir); err != nil {
		return nil, fmt.Errorf("failed to sync repo: %w", err)
	}

	return readBucket(ctx, bucket.Name, bucketDir)
}

// manifest JSON structer
type scoopManifest struct {
	Version     string `json:"version"`
	Description string `json:"description"`
	Homepage    string `json:"homepage"`
	License     any    `json:"license"` // string or object
	URL         any    `json:"url"`     // string or []string
	Hash        any    `json:"hash"`    // string or []string
	Bin         any    `json:"bin"`     // string, []string or [][]string
}

func readBucket(ctx context.Context, bucketName, bucketDir string) ([]normalize.Package, error) {
	manifestDir := filepath.Join(bucketDir, "bucket")

	entries, err := os.ReadDir(manifestDir)
	if err != nil {
		return nil, fmt.Errorf("failed to read bucket dir: %w", err)
	}

	var pkgs []normalize.Package

	for _, entry := range entries {
		if err := ctx.Err(); err != nil {
			return nil, err
		}

		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".json") {
			continue
		}

		pkg, err := readManifest(bucketName, manifestDir, entry.Name())
		if err != nil {
			continue
		}
		pkgs = append(pkgs, pkg)
	}

	return pkgs, nil
}

func readManifest(bucketName, dir, filename string) (normalize.Package, error) {
	path := filepath.Join(dir, filename)

	data, err := os.ReadFile(path)
	if err != nil {
		return normalize.Package{}, fmt.Errorf("failed to read %s: %w", filename, err)
	}

	var m scoopManifest
	if err := json.Unmarshal(data, &m); err != nil {
		return normalize.Package{}, fmt.Errorf("failed to parse %s: %w", filename, err)
	}

	name := strings.TrimSuffix(filename, ".json")
	id := fmt.Sprintf("scoop/%s/%s", bucketName, name)

	return normalize.Package{
		ID:          id,
		Name:        name,
		Version:     m.Version,
		Source:      sourceName,
		Description: m.Description,
		Homepage:    m.Homepage,
		License:     resolveLicense(m.License),
		Installers:  resolveInstallers(m),
		Raw:         data,
	}, nil
}

func resolveLicense(v any) string {
	switch val := v.(type) {
	case string:
		return val
	case map[string]any:
		if id, ok := val["identifier"].(string); ok {
			return id
		}
	}
	return ""
}

func resolveInstallers(m scoopManifest) []normalize.Installer {
	urls := toStringSlice(m.URL)
	hashes := toStringSlice(m.Hash)

	var installers []normalize.Installer
	for i, url := range urls {
		inst := normalize.Installer{
			URL:  url,
			Type: "portable",
		}
		if i < len(hashes) {
			inst.Hash = hashes[i]
		}
		installers = append(installers, inst)
	}
	return installers
}

func toStringSlice(v any) []string {
	switch val := v.(type) {
	case string:
		return []string{val}
	case []any:
		var result []string
		for _, item := range val {
			if s, ok := item.(string); ok {
				result = append(result, s)
			}
		}
		return result
	}
	return nil
}
