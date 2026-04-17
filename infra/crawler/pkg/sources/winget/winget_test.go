package winget

import (
	"archive/zip"
	"bytes"
	"context"
	"errors"
	"fmt"
	"io"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"sync/atomic"
	"testing"
)

func TestWingetManifestPathHelpers(t *testing.T) {
	t.Parallel()

	parts, err := wingetManifestPathParts("Microsoft.WindowsTerminal", "1.9.1942.0", "Microsoft.WindowsTerminal.yaml")
	if err != nil {
		t.Fatalf("wingetManifestPathParts() error = %v", err)
	}

	if got, want := strings.Join(parts, "/"), "manifests/m/Microsoft/WindowsTerminal/1.9.1942.0/Microsoft.WindowsTerminal.yaml"; got != want {
		t.Fatalf("wingetManifestPathParts() = %q, want %q", got, want)
	}

	url, err := wingetManifestURL("Microsoft.WindowsTerminal", "1.9.1942.0", "Microsoft.WindowsTerminal.installer.yaml")
	if err != nil {
		t.Fatalf("wingetManifestURL() error = %v", err)
	}

	if got, want := url, "https://raw.githubusercontent.com/microsoft/winget-pkgs/master/manifests/m/Microsoft/WindowsTerminal/1.9.1942.0/Microsoft.WindowsTerminal.installer.yaml"; got != want {
		t.Fatalf("wingetManifestURL() = %q, want %q", got, want)
	}
}

func TestClassifyWingetPackageSkip(t *testing.T) {
	t.Parallel()

	notFoundErr := fmt.Errorf("failed to fetch winget root manifest for Foo.Bar: %w", nonRetryableError{err: wingetDownloadStatusError{URL: "https://example.invalid", StatusCode: http.StatusNotFound}})
	if got, want := classifyWingetPackageSkip(notFoundErr), "missing_manifest_404"; got != want {
		t.Fatalf("classifyWingetPackageSkip(notFoundErr) = %q, want %q", got, want)
	}

	validationErr := errors.New("winget package Foo.Bar is missing installer manifest")
	if got, want := classifyWingetPackageSkip(validationErr), "missing_installer_manifest"; got != want {
		t.Fatalf("classifyWingetPackageSkip(validationErr) = %q, want %q", got, want)
	}
}

func TestDownloadUsesETagCache(t *testing.T) {
	t.Parallel()

	dir := t.TempDir()
	var calls int32
	var mu sync.Mutex
	var ifNoneMatch []string
	var unexpectedCalls int32
	const etagValue = `"abc123"`

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		call := atomic.AddInt32(&calls, 1)

		mu.Lock()
		ifNoneMatch = append(ifNoneMatch, r.Header.Get("If-None-Match"))
		mu.Unlock()

		switch call {
		case 1:
			w.Header().Set("ETag", etagValue)
			w.WriteHeader(http.StatusOK)
			_, _ = w.Write([]byte("msix-bytes"))
		case 2:
			w.WriteHeader(http.StatusNotModified)
		default:
			atomic.StoreInt32(&unexpectedCalls, 1)
		}
	}))
	defer server.Close()

	src, err := New(server.Client(), dir)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	dst := filepath.Join(dir, "winget-source.msix")
	if err := src.download(context.Background(), server.URL, dst); err != nil {
		t.Fatalf("download(first) error = %v", err)
	}

	data, err := os.ReadFile(dst)
	if err != nil {
		t.Fatalf("ReadFile(dst) error = %v", err)
	}
	if got, want := string(data), "msix-bytes"; got != want {
		t.Fatalf("downloaded content = %q, want %q", got, want)
	}

	etagData, err := os.ReadFile(dst + ".etag")
	if err != nil {
		t.Fatalf("ReadFile(etag) error = %v", err)
	}
	if got, want := string(etagData), etagValue; got != want {
		t.Fatalf("etag file = %q, want %q", got, want)
	}

	if err := src.download(context.Background(), server.URL, dst); err != nil {
		t.Fatalf("download(second) error = %v", err)
	}

	data, err = os.ReadFile(dst)
	if err != nil {
		t.Fatalf("ReadFile(dst after 304) error = %v", err)
	}
	if got, want := string(data), "msix-bytes"; got != want {
		t.Fatalf("cached content = %q, want %q", got, want)
	}

	if atomic.LoadInt32(&unexpectedCalls) != 0 {
		t.Fatal("unexpected request count")
	}

	mu.Lock()
	gotHeaders := append([]string(nil), ifNoneMatch...)
	mu.Unlock()
	if len(gotHeaders) != 2 {
		t.Fatalf("request count = %d, want 2", len(gotHeaders))
	}
	if got, want := gotHeaders[0], ""; got != want {
		t.Fatalf("first request If-None-Match = %q, want %q", got, want)
	}
	if got, want := gotHeaders[1], etagValue; got != want {
		t.Fatalf("second request If-None-Match = %q, want %q", got, want)
	}
}

func TestDownloadSourceDBExtractsWingetDatabase(t *testing.T) {
	var payload bytes.Buffer
	zipWriter := zip.NewWriter(&payload)
	entry, err := zipWriter.Create("public/Index.db")
	if err != nil {
		t.Fatalf("Create() error = %v", err)
	}
	if _, err := io.WriteString(entry, "winget-index-bytes"); err != nil {
		t.Fatalf("WriteString() error = %v", err)
	}
	if err := zipWriter.Close(); err != nil {
		t.Fatalf("Close() error = %v", err)
	}

	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		w.WriteHeader(http.StatusOK)
		_, _ = w.Write(payload.Bytes())
	}))
	defer server.Close()

	dir := t.TempDir()
	src, err := New(server.Client(), dir)
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	originalSourceURL := sourceURL
	sourceURL = server.URL
	defer func() {
		sourceURL = originalSourceURL
	}()

	outPath := filepath.Join(dir, "staging", "winget_source.db")
	if err := src.DownloadSourceDB(context.Background(), outPath); err != nil {
		t.Fatalf("DownloadSourceDB() error = %v", err)
	}

	data, err := os.ReadFile(outPath)
	if err != nil {
		t.Fatalf("ReadFile() error = %v", err)
	}
	if got, want := string(data), "winget-index-bytes"; got != want {
		t.Fatalf("extracted db = %q, want %q", got, want)
	}
}

func TestExtractDBRejectsPathTraversalEntry(t *testing.T) {
	t.Parallel()

	var payload bytes.Buffer
	zipWriter := zip.NewWriter(&payload)
	entry, err := zipWriter.Create("../Public/index.db")
	if err != nil {
		t.Fatalf("Create() error = %v", err)
	}
	if _, err := io.WriteString(entry, "evil"); err != nil {
		t.Fatalf("WriteString() error = %v", err)
	}
	if err := zipWriter.Close(); err != nil {
		t.Fatalf("Close() error = %v", err)
	}

	msixPath := filepath.Join(t.TempDir(), "winget.msix")
	if err := os.WriteFile(msixPath, payload.Bytes(), 0o644); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}

	r, err := zip.OpenReader(msixPath)
	if err != nil {
		t.Fatalf("OpenReader() error = %v", err)
	}
	defer r.Close()

	err = extractFile(r.File[0], filepath.Join(t.TempDir(), "out.db"))
	if err == nil {
		t.Fatal("extractFile() error = nil, want path traversal error")
	}
	if !strings.Contains(err.Error(), "path traversal") {
		t.Fatalf("extractFile() error = %v, want path traversal rejection", err)
	}
}

func TestExtractDBRejectsDuplicateEntries(t *testing.T) {
	t.Parallel()

	var payload bytes.Buffer
	zipWriter := zip.NewWriter(&payload)
	for _, name := range []string{"Public/index.db", "public/Index.db"} {
		entry, err := zipWriter.Create(name)
		if err != nil {
			t.Fatalf("Create(%q) error = %v", name, err)
		}
		if _, err := io.WriteString(entry, "winget-index-bytes"); err != nil {
			t.Fatalf("WriteString(%q) error = %v", name, err)
		}
	}
	if err := zipWriter.Close(); err != nil {
		t.Fatalf("Close() error = %v", err)
	}

	msixPath := filepath.Join(t.TempDir(), "winget.msix")
	if err := os.WriteFile(msixPath, payload.Bytes(), 0o644); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}

	_, err := extractDB(msixPath, filepath.Join(t.TempDir(), "out.db"))
	if err == nil {
		t.Fatal("extractDB() error = nil, want duplicate entry error")
	}
	if !strings.Contains(err.Error(), "multiple index.db entries") {
		t.Fatalf("extractDB() error = %v, want duplicate entry rejection", err)
	}
}

func TestExtractDBRejectsCorruptEntryAndCleansTemp(t *testing.T) {
	t.Parallel()

	var payload bytes.Buffer
	zipWriter := zip.NewWriter(&payload)
	entryHeader := &zip.FileHeader{Name: "public/index.db", Method: zip.Store}
	entry, err := zipWriter.CreateHeader(entryHeader)
	if err != nil {
		t.Fatalf("CreateHeader() error = %v", err)
	}
	if _, err := io.WriteString(entry, "winget-index-bytes"); err != nil {
		t.Fatalf("WriteString() error = %v", err)
	}
	if err := zipWriter.Close(); err != nil {
		t.Fatalf("Close() error = %v", err)
	}

	corrupted := append([]byte(nil), payload.Bytes()...)
	marker := []byte("winget-index-bytes")
	markerOffset := bytes.Index(corrupted, marker)
	if markerOffset < 0 {
		t.Fatal("payload marker not found")
	}
	corrupted[markerOffset] ^= 0xFF

	dir := t.TempDir()
	msixPath := filepath.Join(dir, "winget.msix")
	if err := os.WriteFile(msixPath, corrupted, 0o644); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}

	dstPath := filepath.Join(dir, "out.db")
	_, err = extractDB(msixPath, dstPath)
	if err == nil {
		t.Fatal("extractDB() error = nil, want checksum failure")
	}
	if got := strings.ToLower(err.Error()); !strings.Contains(got, "checksum") && !strings.Contains(got, "verify zip entry") {
		t.Fatalf("extractDB() error = %v, want checksum verification failure", err)
	}

	if _, statErr := os.Stat(dstPath); !errors.Is(statErr, os.ErrNotExist) {
		t.Fatalf("dst file exists after failed extraction: %v", statErr)
	}

	tempMatches, err := filepath.Glob(filepath.Join(dir, "out.db.*.tmp"))
	if err != nil {
		t.Fatalf("Glob() error = %v", err)
	}
	if len(tempMatches) != 0 {
		t.Fatalf("temporary files left behind: %v", tempMatches)
	}
}
