package winget

import (
	"context"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
	"strings"

	"winbrew/infra/pkg/normalize"
)

const (
	sourceURL       = "https://cdn.winget.microsoft.com/cache/source.msix"
	sourceName      = "winget"
	maxDownloadSize = 2 << 30
)

type Source struct {
	httpClient *http.Client
	cacheDir   string
}

func New(httpClient *http.Client, cacheDir string) (*Source, error) {
	if httpClient == nil {
		return nil, fmt.Errorf("http client cannot be nil")
	}
	if cacheDir == "" {
		return nil, fmt.Errorf("cache dir cannot be empty")
	}
	if err := os.MkdirAll(cacheDir, 0o755); err != nil {
		return nil, fmt.Errorf("failed to create cache dir: %w", err)
	}

	return &Source{
		httpClient: httpClient,
		cacheDir:   cacheDir,
	}, nil
}

func (s *Source) Name() string {
	return sourceName
}

func (s *Source) Fetch(ctx context.Context) ([]normalize.Package, error) {
	if err := ctx.Err(); err != nil {
		return nil, err
	}

	msixPath := filepath.Join(s.cacheDir, "winget-source.msix")

	if err := s.download(ctx, sourceURL, msixPath); err != nil {
		return nil, fmt.Errorf("failed to download winget source: %w", err)
	}

	dbPath, err := extractDB(msixPath, s.cacheDir)
	if err != nil {
		return nil, fmt.Errorf("failed to extract winget db: %w", err)
	}

	return readPackages(ctx, dbPath)
}

func (s *Source) download(ctx context.Context, url, dst string) error {
	req, err := http.NewRequestWithContext(ctx, http.MethodGet, url, nil)
	if err != nil {
		return fmt.Errorf("failed to create request: %w", err)
	}

	if etag, err := os.ReadFile(dst + ".etag"); err == nil {
		if trimmed := strings.TrimSpace(string(etag)); trimmed != "" {
			req.Header.Set("If-None-Match", trimmed)
		}
	}

	resp, err := s.httpClient.Do(req)
	if err != nil {
		return fmt.Errorf("failed to fetch %s: %w", url, err)
	}
	defer resp.Body.Close()

	if resp.StatusCode == http.StatusNotModified {
		if _, err := os.Stat(dst); err != nil {
			return fmt.Errorf("received 304 without cached file: %w", err)
		}
		return nil
	}

	if resp.StatusCode != http.StatusOK {
		return fmt.Errorf("unexpected status %d for %s", resp.StatusCode, url)
	}

	tempFile, err := os.CreateTemp(filepath.Dir(dst), filepath.Base(dst)+".*.tmp")
	if err != nil {
		return fmt.Errorf("failed to create temp file: %w", err)
	}
	tempPath := tempFile.Name()

	n, err := io.Copy(tempFile, io.LimitReader(resp.Body, maxDownloadSize+1))
	if err != nil {
		_ = tempFile.Close()
		_ = os.Remove(tempPath)
		return fmt.Errorf("failed to write file: %w", err)
	}
	if n > maxDownloadSize {
		_ = tempFile.Close()
		_ = os.Remove(tempPath)
		return fmt.Errorf("download exceeds %d bytes", maxDownloadSize)
	}

	if err := tempFile.Close(); err != nil {
		_ = os.Remove(tempPath)
		return fmt.Errorf("failed to close temp file: %w", err)
	}

	if err := os.Rename(tempPath, dst); err != nil {
		_ = os.Remove(tempPath)
		return fmt.Errorf("failed to move downloaded file into place: %w", err)
	}

	if etag := resp.Header.Get("ETag"); etag != "" {
		_ = os.WriteFile(dst+".etag", []byte(etag), 0o644)
	}

	return nil
}
