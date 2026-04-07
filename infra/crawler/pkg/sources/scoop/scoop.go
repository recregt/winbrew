package scoop

import (
	"bufio"
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"os"
	"os/exec"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"golang.org/x/sync/errgroup"

	"infra/crawler/internal/retry"
	"infra/crawler/pkg/normalize"
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

const maxManifestSize = 1 << 20

var manifestReadSemaphore = make(chan struct{}, 16)

func (s *Source) Close() error {
	return nil
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

	buckets := make([]Bucket, 0, len(defaultBuckets)+len(extra))
	seen := make(map[string]struct{}, len(defaultBuckets)+len(extra))
	for _, bucket := range append(append([]Bucket(nil), defaultBuckets...), extra...) {
		if _, ok := seen[bucket.Name]; ok {
			continue
		}
		seen[bucket.Name] = struct{}{}
		buckets = append(buckets, bucket)
	}

	return &Source{
		buckets:  buckets,
		cacheDir: cacheDir,
	}, nil
}

func (s *Source) Name() string {
	return sourceName
}

func (s *Source) WriteJSONL(ctx context.Context, w io.Writer, maxAttempts int, backoff time.Duration) (err error) {
	writer, flush := bufferJSONLWriter(w)
	defer func() {
		if flushErr := flush(); err == nil && flushErr != nil {
			err = fmt.Errorf("failed to flush JSONL writer: %w", flushErr)
		}
	}()

	enc := json.NewEncoder(writer)
	type bucketResult struct {
		bucket Bucket
		dir    string
		err    error
	}

	results := make([]bucketResult, len(s.buckets))
	group, groupCtx := errgroup.WithContext(ctx)

	for i, bucket := range s.buckets {
		i, bucket := i, bucket
		dir := filepath.Join(s.cacheDir, bucket.Name)

		group.Go(func() error {
			err := retry.Do(groupCtx, maxAttempts, backoff, func() error {
				return syncRepo(groupCtx, bucket.URL, dir)
			})
			if err != nil {
				err = fmt.Errorf("bucket %s: %w", bucket.Name, err)
			}

			results[i] = bucketResult{bucket: bucket, dir: dir, err: err}
			return err
		})
	}

	if err := group.Wait(); err != nil {
		return err
	}

	succeeded := 0
	failed := 0
	var lastErr error

	for _, result := range results {
		if result.err != nil {
			failed++
			lastErr = result.err
			slog.Error("bucket sync failed", "bucket", result.bucket.Name, "err", result.err)
			continue
		}

		if err := writeBucketJSONL(ctx, enc, result.bucket.Name, result.dir); err != nil {
			if ctxErr := ctx.Err(); ctxErr != nil {
				return ctxErr
			}

			failed++
			lastErr = err
			slog.Error("bucket write failed", "bucket", result.bucket.Name, "err", err)
			continue
		}

		succeeded++
	}

	if failed > 0 {
		return fmt.Errorf("partial failure: %d succeeded, %d failed, last error: %w", succeeded, failed, lastErr)
	}

	return nil
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

	manifestNames := make([]string, 0, len(entries))
	for _, entry := range entries {
		if entry.IsDir() || !strings.HasSuffix(entry.Name(), ".json") {
			continue
		}

		manifestNames = append(manifestNames, entry.Name())
	}

	if len(manifestNames) == 0 {
		return nil
	}

	type manifestResult struct {
		manifest string
		pkg      packageSnapshot
		err      error
	}

	results := make([]manifestResult, len(manifestNames))
	jobs := make(chan int)
	workerCount := 4
	if len(manifestNames) < workerCount {
		workerCount = len(manifestNames)
	}
	slog.Debug("starting manifest workers", "bucket", bucketName, "manifests", len(manifestNames), "workers", workerCount)

	var wg sync.WaitGroup
	wg.Add(workerCount)
	for i := 0; i < workerCount; i++ {
		workerID := i
		go func() {
			defer wg.Done()
			slog.Debug("manifest worker started", "bucket", bucketName, "worker", workerID)
			for idx := range jobs {
				select {
				case <-ctx.Done():
					return
				default:
				}

				manifest := manifestNames[idx]
				pkg, err := readManifest(ctx, bucketName, manifestDir, manifest)
				if err != nil {
					results[idx] = manifestResult{manifest: manifest, err: err}
					continue
				}

				results[idx] = manifestResult{manifest: manifest, pkg: packageSnapshotFromPackage(pkg)}
			}
		}()
	}

	for idx := range manifestNames {
		select {
		case <-ctx.Done():
			close(jobs)
			wg.Wait()
			return ctx.Err()
		case jobs <- idx:
		}
	}
	close(jobs)
	wg.Wait()

	if err := ctx.Err(); err != nil {
		return err
	}

	for _, result := range results {
		if result.err != nil {
			slog.Warn("skipping manifest", "bucket", bucketName, "manifest", result.manifest, "err", result.err)
			continue
		}
		if err := enc.Encode(result.pkg); err != nil {
			return fmt.Errorf("failed to encode package %s: %w", result.pkg.ID, err)
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

func readManifest(ctx context.Context, bucketName, dir, filename string) (normalize.Package, error) {
	select {
	case manifestReadSemaphore <- struct{}{}:
	case <-ctx.Done():
		return normalize.Package{}, ctx.Err()
	}
	defer func() {
		<-manifestReadSemaphore
	}()

	path := filepath.Join(dir, filename)
	info, err := os.Stat(path)
	if err != nil {
		return normalize.Package{}, fmt.Errorf("failed to stat %s: %w", filename, err)
	}
	if info.Size() > maxManifestSize {
		return normalize.Package{}, fmt.Errorf("manifest too large: %d bytes", info.Size())
	}

	file, err := os.Open(path)
	if err != nil {
		return normalize.Package{}, fmt.Errorf("failed to open %s: %w", filename, err)
	}
	defer file.Close()

	var raw bytes.Buffer
	var m scoopManifest
	if err := json.NewDecoder(io.TeeReader(file, &raw)).Decode(&m); err != nil {
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
		Raw:         append(json.RawMessage(nil), raw.Bytes()...),
	}, nil
}

func bufferJSONLWriter(w io.Writer) (io.Writer, func() error) {
	if bw, ok := w.(*bufio.Writer); ok {
		return bw, bw.Flush
	}

	bw := bufio.NewWriterSize(w, 64*1024)
	return bw, bw.Flush
}

func resolveLicense(v any) string {
	switch val := v.(type) {
	case string:
		return val
	case map[string]any:
		if id, ok := val["identifier"].(string); ok {
			return id
		}
		if url, ok := val["url"].(string); ok {
			return url
		}
	}
	return ""
}

func resolveInstallers(m scoopManifest) []normalize.Installer {
	if len(m.Architecture) > 0 {
		var installers []normalize.Installer
		for _, arch := range []string{"x64", "amd64", "x86", "386", "arm64", "aarch64", "any", "neutral"} {
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
