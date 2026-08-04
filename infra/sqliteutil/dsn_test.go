package sqliteutil

import (
	"net/url"
	"path/filepath"
	"runtime"
	"strings"
	"testing"
)

func TestDSNPrefixesWindowsDrivePath(t *testing.T) {
	t.Parallel()

	dbPath := filepath.Join(t.TempDir(), "winget_source.db")
	if runtime.GOOS == "windows" {
		dbPath = `C:\Users\recregt\AppData\Local\winbrew\winget\winget_source.db`
	}

	dsn, err := DSN(dbPath)
	if err != nil {
		t.Fatalf("DSN() error = %v", err)
	}

	absPath, err := filepath.Abs(dbPath)
	if err != nil {
		t.Fatalf("filepath.Abs() error = %v", err)
	}
	wantPath := filepath.ToSlash(absPath)
	if runtime.GOOS == "windows" && len(wantPath) >= 2 && wantPath[1] == ':' {
		wantPath = "/" + wantPath
	}

	if got, want := dsn, (&url.URL{Scheme: "file", Path: wantPath, RawQuery: "mode=ro"}).String(); got != want {
		t.Fatalf("DSN() = %q, want %q", got, want)
	}
	if runtime.GOOS == "windows" && !strings.HasPrefix(dsn, "file:///C:/") {
		t.Fatalf("DSN() = %q, want Windows drive path to keep the leading slash", dsn)
	}
}
