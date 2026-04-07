package db

import (
	"context"
	"encoding/json"
	"path/filepath"
	"testing"

	"zombiezen.com/go/sqlite"
	"zombiezen.com/go/sqlite/sqlitex"

	"winbrew/infra/pkg/normalize"
)

func TestWritePackagesReplacesInstallers(t *testing.T) {
	t.Parallel()

	dir := t.TempDir()
	writer, err := Open(filepath.Join(dir, "catalog.db"))
	if err != nil {
		t.Fatalf("Open() error = %v", err)
	}
	defer writer.Close()

	ctx := context.Background()
	packageID := "scoop/example"

	first := []normalize.Package{{
		ID:      packageID,
		Name:    "example",
		Version: "1.0.0",
		Raw:     json.RawMessage(`{"version":"1.0.0"}`),
		Installers: []normalize.Installer{{
			URL:  "https://example.invalid/one.zip",
			Hash: "hash-one",
			Type: "portable",
		}},
	}}
	if err := writer.WritePackages(ctx, first); err != nil {
		t.Fatalf("WritePackages(first) error = %v", err)
	}

	second := []normalize.Package{{
		ID:      packageID,
		Name:    "example",
		Version: "2.0.0",
		Raw:     json.RawMessage(`{"version":"2.0.0"}`),
		Installers: []normalize.Installer{{
			URL:  "https://example.invalid/two.zip",
			Hash: "hash-two",
			Type: "portable",
		}},
	}}
	if err := writer.WritePackages(ctx, second); err != nil {
		t.Fatalf("WritePackages(second) error = %v", err)
	}

	conn, err := sqlite.OpenConn(filepath.Join(dir, "catalog.db"), sqlite.OpenReadOnly)
	if err != nil {
		t.Fatalf("OpenConn() error = %v", err)
	}
	defer conn.Close()

	var installerCount int64
	var version string
	var raw string
	err = sqlitex.ExecuteTransient(conn, `
SELECT
	(SELECT COUNT(*) FROM catalog_installers WHERE package_id = ?),
	(SELECT version FROM catalog_packages WHERE id = ?),
	(SELECT raw FROM catalog_packages_raw WHERE package_id = ?)
`, &sqlitex.ExecOptions{
		Args: []any{packageID, packageID, packageID},
		ResultFunc: func(stmt *sqlite.Stmt) error {
			installerCount = stmt.ColumnInt64(0)
			version = stmt.ColumnText(1)
			raw = stmt.ColumnText(2)
			return nil
		},
	})
	if err != nil {
		t.Fatalf("query error = %v", err)
	}

	if got, want := installerCount, int64(1); got != want {
		t.Fatalf("installer count = %d, want %d", got, want)
	}
	if got, want := version, "2.0.0"; got != want {
		t.Fatalf("version = %q, want %q", got, want)
	}
	if got, want := raw, `{"version":"2.0.0"}`; got != want {
		t.Fatalf("raw = %q, want %q", got, want)
	}
}
