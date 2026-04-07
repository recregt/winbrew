package db

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"sync"

	"zombiezen.com/go/sqlite"
	"zombiezen.com/go/sqlite/sqlitex"

	"winbrew/infra/pkg/normalize"
)

type Writer struct {
	conn *sqlite.Conn
	mu   sync.Mutex
}

func Open(path string) (*Writer, error) {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return nil, fmt.Errorf("failed to create database directory: %w", err)
	}

	for _, suffix := range []string{"", "-wal", "-shm"} {
		_ = os.Remove(path + suffix)
	}

	conn, err := sqlite.OpenConn(path, sqlite.OpenReadWrite|sqlite.OpenCreate)
	if err != nil {
		return nil, fmt.Errorf("failed to open database: %w", err)
	}
	if err := sqlitex.ExecuteTransient(conn, "PRAGMA journal_mode=WAL;", nil); err != nil {
		conn.Close()
		return nil, fmt.Errorf("failed to enable WAL mode: %w", err)
	}
	if err := sqlitex.ExecuteTransient(conn, "PRAGMA synchronous=NORMAL;", nil); err != nil {
		conn.Close()
		return nil, fmt.Errorf("failed to set synchronous mode: %w", err)
	}
	if err := sqlitex.ExecuteTransient(conn, "PRAGMA foreign_keys=ON;", nil); err != nil {
		conn.Close()
		return nil, fmt.Errorf("failed to enable foreign keys: %w", err)
	}
	if err := sqlitex.ExecScript(conn, schema); err != nil {
		conn.Close()
		return nil, fmt.Errorf("failed to apply schema: %w", err)
	}
	return &Writer{conn: conn}, nil
}

func (w *Writer) Close() error {
	return w.conn.Close()
}

func (w *Writer) WritePackages(ctx context.Context, pkgs []normalize.Package) (err error) {
	w.mu.Lock()
	defer w.mu.Unlock()

	defer sqlitex.Save(w.conn)(&err)

	for _, pkg := range pkgs {
		if err := ctx.Err(); err != nil {
			return err
		}
		if err := w.writePackage(pkg); err != nil {
			return err
		}
	}
	return nil
}

func (w *Writer) writePackage(pkg normalize.Package) error {
	raw, err := json.Marshal(pkg.Raw)
	if err != nil {
		return fmt.Errorf("failed to marshal raw for %s: %w", pkg.ID, err)
	}

	err = sqlitex.ExecuteTransient(w.conn,
		`INSERT INTO catalog_packages(id, name, version, description, homepage, license, publisher)
		 VALUES (?, ?, ?, ?, ?, ?, ?)
		 ON CONFLICT(id) DO UPDATE SET
		   name=excluded.name,
		   version=excluded.version,
		   description=excluded.description,
		   homepage=excluded.homepage,
		   license=excluded.license,
		   publisher=excluded.publisher`,
		&sqlitex.ExecOptions{Args: []any{
			pkg.ID, pkg.Name, pkg.Version,
			pkg.Description, pkg.Homepage, pkg.License, pkg.Publisher,
		}},
	)
	if err != nil {
		return fmt.Errorf("failed to insert package %s: %w", pkg.ID, err)
	}

	err = sqlitex.ExecuteTransient(w.conn,
		`INSERT INTO catalog_packages_raw(package_id, raw)
		 VALUES (?, ?)
		 ON CONFLICT(package_id) DO UPDATE SET
		   raw=excluded.raw`,
		&sqlitex.ExecOptions{Args: []any{pkg.ID, string(raw)}},
	)
	if err != nil {
		return fmt.Errorf("failed to insert raw package %s: %w", pkg.ID, err)
	}

	err = sqlitex.ExecuteTransient(w.conn,
		`DELETE FROM catalog_installers WHERE package_id = ?`,
		&sqlitex.ExecOptions{Args: []any{pkg.ID}},
	)
	if err != nil {
		return fmt.Errorf("failed to delete old installers for %s: %w", pkg.ID, err)
	}

	for _, inst := range pkg.Installers {
		err = sqlitex.ExecuteTransient(w.conn,
			`INSERT INTO catalog_installers(package_id, url, hash, arch, type)
			 VALUES (?, ?, ?, ?, ?)`,
			&sqlitex.ExecOptions{Args: []any{pkg.ID, inst.URL, inst.Hash, inst.Arch, inst.Type}},
		)
		if err != nil {
			return fmt.Errorf("failed to insert installer for %s: %w", pkg.ID, err)
		}
	}

	return nil
}
