package winget

import (
	"archive/zip"
	"bytes"
	"context"
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

	if got, want := url, "https://raw.githubusercontent.com/microsoft/winget-pkgs/main/manifests/m/Microsoft/WindowsTerminal/1.9.1942.0/Microsoft.WindowsTerminal.installer.yaml"; got != want {
		t.Fatalf("wingetManifestURL() = %q, want %q", got, want)
	}
}

func TestWingetManifestResolutionSingleton(t *testing.T) {
	t.Parallel()

	manifest, err := parseWingetManifest([]byte(`
PackageIdentifier: Microsoft.WindowsTerminal
PackageVersion: 1.9.1942.0
PackageName: Windows Terminal
Publisher: Microsoft Corporation
ShortDescription: Modern terminal
License: MIT
Homepage: https://example.invalid
Installers:
  - Architecture: x64
    InstallerType: msix
    InstallerUrl: https://example.invalid/terminal.msixbundle
    InstallerSha256: ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789
    Scope: user
  - Architecture: arm
    InstallerType: zip
    InstallerUrl: https://example.invalid/terminal.zip
    InstallerSha256: 0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF
    NestedInstallerType: portable
ManifestType: singleton
ManifestVersion: 1.12.0
`))
	if err != nil {
		t.Fatalf("parseWingetManifest() error = %v", err)
	}

	pkg, err := buildWingetPackageSnapshot(wingetIndexRow{
		id:        "Microsoft.WindowsTerminal",
		name:      "Windows Terminal",
		version:   "1.9.1942.0",
		publisher: "Microsoft Corporation",
	}, manifest, nil, nil)
	if err != nil {
		t.Fatalf("buildWingetPackageSnapshot() error = %v", err)
	}

	if got, want := pkg.ID, "winget/Microsoft.WindowsTerminal"; got != want {
		t.Fatalf("package id = %q, want %q", got, want)
	}
	if got, want := pkg.Name, "Windows Terminal"; got != want {
		t.Fatalf("package name = %q, want %q", got, want)
	}
	if got, want := pkg.Publisher, "Microsoft Corporation"; got != want {
		t.Fatalf("package publisher = %q, want %q", got, want)
	}
	if got, want := pkg.Installers[0].Scope, "installed"; got != want {
		t.Fatalf("installer scope = %q, want %q", got, want)
	}
	if got, want := pkg.Installers[1].Arch, ""; got != want {
		t.Fatalf("installer arch = %q, want %q", got, want)
	}
	if got, want := pkg.Installers[1].NestedKind, "portable"; got != want {
		t.Fatalf("installer nested kind = %q, want %q", got, want)
	}
}

func TestWingetManifestResolutionMultiFile(t *testing.T) {
	t.Parallel()

	root, err := parseWingetManifest([]byte(`
PackageIdentifier: Contoso.App
PackageVersion: 2.3.4
DefaultLocale: en-US
ManifestType: version
ManifestVersion: 1.12.0
`))
	if err != nil {
		t.Fatalf("parseWingetManifest(root) error = %v", err)
	}

	locale, err := parseWingetManifest([]byte(`
PackageIdentifier: Contoso.App
PackageVersion: 2.3.4
PackageLocale: en-US
Publisher: Contoso Ltd.
PackageName: Contoso App
ShortDescription: Contoso app
Homepage: https://contoso.example
License: MIT
ManifestType: defaultLocale
ManifestVersion: 1.12.0
`))
	if err != nil {
		t.Fatalf("parseWingetManifest(locale) error = %v", err)
	}

	installer, err := parseWingetManifest([]byte(`
PackageIdentifier: Contoso.App
PackageVersion: 2.3.4
Installers:
  - Architecture: x64
    InstallerType: exe
    InstallerUrl: https://example.invalid/app.exe
    InstallerSha256: 0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF
    Scope: machine
ManifestType: installer
ManifestVersion: 1.12.0
`))
	if err != nil {
		t.Fatalf("parseWingetManifest(installer) error = %v", err)
	}

	pkg, err := buildWingetPackageSnapshot(wingetIndexRow{
		id:        "Contoso.App",
		name:      "Contoso App",
		version:   "2.3.4",
		publisher: "Contoso Ltd.",
	}, root, &locale, &installer)
	if err != nil {
		t.Fatalf("buildWingetPackageSnapshot() error = %v", err)
	}

	if got, want := pkg.Description, "Contoso app"; got != want {
		t.Fatalf("package description = %q, want %q", got, want)
	}
	if got, want := pkg.Installers[0].Scope, "provisioned"; got != want {
		t.Fatalf("installer scope = %q, want %q", got, want)
	}
	if got, want := pkg.Installers[0].Arch, "x64"; got != want {
		t.Fatalf("installer arch = %q, want %q", got, want)
	}
	if got, want := pkg.Installers[0].Type, "exe"; got != want {
		t.Fatalf("installer type = %q, want %q", got, want)
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
