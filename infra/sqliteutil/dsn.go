// Package sqliteutil provides small helpers shared by the Go pipeline's
// SQLite consumers (crawler and publisher).
package sqliteutil

import (
	"fmt"
	"net/url"
	"path/filepath"
)

// DSN builds a read-only file: DSN for modernc.org/sqlite from a filesystem
// path, handling the Windows-drive-letter/URL-path quirk.
func DSN(dbPath string) (string, error) {
	absPath, err := filepath.Abs(dbPath)
	if err != nil {
		return "", fmt.Errorf("failed to resolve sqlite database path: %w", err)
	}
	uriPath := filepath.ToSlash(absPath)
	if len(uriPath) >= 2 && uriPath[1] == ':' {
		uriPath = "/" + uriPath
	}

	return (&url.URL{
		Scheme:   "file",
		Path:     uriPath,
		RawQuery: "mode=ro",
	}).String(), nil
}
