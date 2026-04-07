package scoop

import (
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"time"

	"golang.org/x/sync/errgroup"

	"winbrew/infra/internal/retry"
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
	if _, err := exec.LookPath("git"); err != nil {
		return nil, fmt.Errorf("git executable not found in PATH: %w", err)
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

func (s *Source) WriteJSONL(ctx context.Context, w io.Writer, maxAttempts int, backoff time.Duration) error {
	enc := json.NewEncoder(w)
	type bucketState struct {
		bucket Bucket
		dir    string
		ready  chan error
	}

	states := make([]bucketState, 0, len(s.buckets))
	group, groupCtx := errgroup.WithContext(ctx)

	for _, bucket := range s.buckets {
		bucket := bucket
		state := bucketState{
			bucket: bucket,
			dir:    filepath.Join(s.cacheDir, bucket.Name),
			ready:  make(chan error, 1),
		}
		states = append(states, state)

		group.Go(func() error {
			err := retry.Do(groupCtx, maxAttempts, backoff, func() error {
				return syncRepo(groupCtx, bucket.URL, state.dir)
			})
			if err != nil {
				err = fmt.Errorf("bucket %s: %w", bucket.Name, err)
			}

			state.ready <- err
			close(state.ready)
			return err
		})
	}

	for _, state := range states {
		err := <-state.ready
		if err != nil {
			return err
		}

		if err := writeBucketJSONL(ctx, enc, state.bucket.Name, state.dir); err != nil {
			return fmt.Errorf("bucket %s: %w", state.bucket.Name, err)
		}
	}

	return group.Wait()
}

type packageSnapshot struct {
	ID          string              `json:"id"`
	Name        string              `json:"name"`
	Version     string              `json:"version"`
	Description string              `json:"description,omitempty"`
	Homepage    string              `json:"homepage,omitempty"`
	License     string              `json:"license,omitempty"`
	Publisher   string              `json:"publisher,omitempty"`
	Installers  []installerSnapshot `json:"installers,omitempty"`
}

type installerSnapshot struct {
	URL  string `json:"url"`
	Hash string `json:"hash,omitempty"`
	Arch string `json:"arch,omitempty"`
	Type string `json:"type"`
}

func packageSnapshotFromPackage(pkg normalize.Package) packageSnapshot {
	installers := make([]installerSnapshot, 0, len(pkg.Installers))
	for _, installer := range pkg.Installers {
		installers = append(installers, installerSnapshot{
			URL:  installer.URL,
			Hash: installer.Hash,
			Arch: installer.Arch,
			Type: installer.Type,
		})
	}

	return packageSnapshot{
		ID:          pkg.ID,
		Name:        pkg.Name,
		Version:     pkg.Version,
		Description: pkg.Description,
		Homepage:    pkg.Homepage,
		License:     pkg.License,
		Publisher:   pkg.Publisher,
		Installers:  installers,
	}
}

func writeBucketJSONL(ctx context.Context, enc *json.Encoder, bucketName, bucketDir string) error {
	manifestDir := filepath.Join(bucketDir, "bucket")

	if _, err := os.Stat(manifestDir); os.IsNotExist(err) {
		slog.Warn("bucket has no manifest dir", "bucket", bucketName)
		return nil
	} else if err != nil {
		return fmt.Errorf("failed to stat bucket dir: %w", err)
	}

	entries, err := os.ReadDir(manifestDir)
	if err != nil {
		return fmt.Errorf("failed to read bucket dir: %w", err)
	}

	for _, entry := range entries {
		if err := ctx.Err(); err != nil {
			return err
		}

		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".json") {
			continue
		}

		pkg, err := readManifest(bucketName, manifestDir, entry.Name())
		if err != nil {
			slog.Warn("skipping manifest", "bucket", bucketName, "manifest", entry.Name(), "err", err)
			continue
		}
		if err := enc.Encode(packageSnapshotFromPackage(pkg)); err != nil {
			return fmt.Errorf("failed to encode package %s: %w", pkg.ID, err)
		}
	}

	return nil
}

// manifest JSON structure
type scoopManifest struct {
	Version      string               `json:"version"`
	Description  string               `json:"description"`
	Homepage     string               `json:"homepage"`
	License      any                  `json:"license"` // string or object
	URL          any                  `json:"url"`     // string or []string
	Hash         any                  `json:"hash"`    // string or []string
	Bin          any                  `json:"bin"`     // string, []string or [][]string
	Architecture map[string]archBlock `json:"architecture"`
}

type archBlock struct {
	URL  any `json:"url"`
	Hash any `json:"hash"`
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
	if len(m.Architecture) > 0 {
		var installers []normalize.Installer
		for _, arch := range []string{"x64", "x86", "arm64"} {
			block, ok := m.Architecture[arch]
			if !ok {
				continue
			}

			urls := toStringSlice(block.URL)
			hashes := toStringSlice(block.Hash)
			for i, url := range urls {
				inst := normalize.Installer{
					URL:  url,
					Type: "portable",
					Arch: arch,
				}
				if i < len(hashes) {
					inst.Hash = hashes[i]
				}
				installers = append(installers, inst)
			}
		}
		if len(installers) > 0 {
			return installers
		}
	}

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
