package winget

import (
	"context"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"os"
	"path/filepath"
	"strings"
)

var sourceURL = "https://cdn.winget.microsoft.com/cache/source.msix"

const (
	sourceName      = "winget"
	maxDownloadSize = 1 << 30 // 1 GiB
)

type Source struct {
	httpClient *http.Client
	cacheDir   string
}

func (s *Source) Close() error {
	if s.httpClient != nil {
		s.httpClient.CloseIdleConnections()
	}
	return nil
}

func New(httpClient *http.Client, cacheDir string) (*Source, error) {
	if httpClient == nil {
		return nil, fmt.Errorf("http client cannot be nil")
	}
	if cacheDir == "" {
		return nil, fmt.Errorf("cache dir cannot be empty")
	}
	if err := os.MkdirAll(cacheDir, 0o750); err != nil {
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

func (s *Source) DownloadSourceDB(ctx context.Context, dst string) error {
	if err := ctx.Err(); err != nil {
		return err
	}
	if strings.TrimSpace(dst) == "" {
		return fmt.Errorf("destination path cannot be empty")
	}

	if err := os.MkdirAll(filepath.Dir(dst), 0o750); err != nil {
		return fmt.Errorf("failed to create destination dir: %w", err)
	}

	msixPath := filepath.Join(s.cacheDir, "winget-source.msix")

	if err := s.download(ctx, sourceURL, msixPath); err != nil {
		return fmt.Errorf("failed to download winget source: %w", err)
	}

	if _, err := extractDB(msixPath, dst); err != nil {
		return fmt.Errorf("failed to extract winget db: %w", err)
	}

	return nil
}

func (s *Source) download(ctx context.Context, url, dst string) error {
	slog.Debug("starting winget download", "url", url, "dst", dst)

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
			return nonRetryableError{err: fmt.Errorf("received 304 without cached file: %w", err)}
		}
		return nil
	}

	if resp.StatusCode != http.StatusOK {
		err := fmt.Errorf("unexpected status %d for %s", resp.StatusCode, url)
		if resp.StatusCode >= http.StatusBadRequest && resp.StatusCode < http.StatusInternalServerError && resp.StatusCode != http.StatusTooManyRequests {
			return nonRetryableError{err: err}
		}
		return err
	}

	tempFile, err := os.CreateTemp(filepath.Dir(dst), filepath.Base(dst)+".*.tmp")
	if err != nil {
		return fmt.Errorf("failed to create temp file: %w", err)
	}
	tempPath := tempFile.Name()
	defer func() {
		_ = tempFile.Close()
		_ = os.Remove(tempPath)
	}()

	buf := make([]byte, 32*1024)
	body := io.Reader(resp.Body)
	if resp.ContentLength > 0 {
		body = &progressReader{url: url, total: resp.ContentLength, reader: resp.Body}
	}

	n, err := io.CopyBuffer(tempFile, io.LimitReader(body, maxDownloadSize+1), buf)
	if err != nil {
		return fmt.Errorf("failed to write file: %w", err)
	}
	if n > maxDownloadSize {
		return fmt.Errorf("download exceeds %d bytes", maxDownloadSize)
	}

	if err := tempFile.Close(); err != nil {
		return fmt.Errorf("failed to close temp file: %w", err)
	}

	if err := os.Rename(tempPath, dst); err != nil {
		return fmt.Errorf("failed to move downloaded file into place: %w", err)
	}

	if etag := resp.Header.Get("ETag"); etag != "" {
		_ = os.WriteFile(dst+".etag", []byte(etag), 0o644)
	}

	slog.Debug("completed winget download", "url", url, "dst", dst, "bytes", n)

	return nil
}

type nonRetryableError struct {
	err error
}

func (e nonRetryableError) Error() string {
	return e.err.Error()
}

func (e nonRetryableError) Unwrap() error {
	return e.err
}

func (e nonRetryableError) NonRetryable() bool {
	return true
}

type progressReader struct {
	url     string
	total   int64
	read    int64
	reader  io.Reader
	nextLog int64
}

func (pr *progressReader) Read(p []byte) (int, error) {
	n, err := pr.reader.Read(p)
	if n <= 0 {
		return n, err
	}

	pr.read += int64(n)
	if pr.nextLog == 0 {
		pr.nextLog = 16 << 20
	}
	if pr.read >= pr.nextLog || err == io.EOF {
		if pr.total > 0 {
			slog.Debug("winget download progress", "url", pr.url, "downloaded", pr.read, "total", pr.total)
		} else {
			slog.Debug("winget download progress", "url", pr.url, "downloaded", pr.read)
		}
		for pr.read >= pr.nextLog {
			pr.nextLog += 16 << 20
		}
	}

	return n, err
}
