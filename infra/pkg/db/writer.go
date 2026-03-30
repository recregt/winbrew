package db

import (
	"context"
	"encoding/json"
	"fmt"

	"zombiezen.com/go/sqlite"
	"zombiezen.com/go/sqlite/sqlitex"

	"github.com/recregt/winbrew-infra/pkg/normalize"
)

type Writer struct {
	conn *sqlite.Conn
}

func Open(path string) (*Writer, error) {
	conn, err := sqlite.OpenConn(path, sqlite.OpenReadWrite|sqlite.OpenCreate)
	if err != nil {
		return nil, fmt.Errorf("failed to open database: %w", err)
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

func (w *Writer) WritePackages(ctx context.Context, pkgs []normalize.Package) error {
	if err := sqlitex.ExecuteTransient(w.conn, "BEGIN", nil); err != nil {
		return fmt.Errorf("failed to begin transaction: %w", err)
	}
	committed := false
	defer func() {
		if !committed {
			_ = sqlitex.ExecuteTransient(w.conn, "ROLLBACK", nil)
		}
	}()

	for _, pkg := range pkgs {
		if err := ctx.Err(); err != nil {
			return err
		}
		if err := w.writePackage(pkg); err != nil {
			return err
		}
	}

	if err := ctx.Err(); err != nil {
		return err
	}
	if err := sqlitex.ExecuteTransient(w.conn, "COMMIT", nil); err != nil {
		return fmt.Errorf("failed to commit transaction: %w", err)
	}
	committed = true
	return nil
}

func (w *Writer) writePackage(pkg normalize.Package) error {
	raw, err := json.Marshal(pkg.Raw)
	if err != nil {
		return fmt.Errorf("failed to marshal raw for %s: %w", pkg.ID, err)
	}

	err = sqlitex.ExecuteTransient(w.conn,
		`INSERT INTO packages(id, name, version, source, description, homepage, license, publisher, raw)
		 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
		 ON CONFLICT(id) DO UPDATE SET
		   name=excluded.name,
		   version=excluded.version,
		   source=excluded.source,
		   description=excluded.description,
		   homepage=excluded.homepage,
		   license=excluded.license,
		   publisher=excluded.publisher,
		   raw=excluded.raw`,
		&sqlitex.ExecOptions{Args: []any{
			pkg.ID, pkg.Name, pkg.Version, pkg.Source,
			pkg.Description, pkg.Homepage, pkg.License, pkg.Publisher,
			string(raw),
		}},
	)
	if err != nil {
		return fmt.Errorf("failed to insert package %s: %w", pkg.ID, err)
	}

	for _, inst := range pkg.Installers {
		err = sqlitex.ExecuteTransient(w.conn,
			`INSERT OR IGNORE INTO installers(package_id, url, hash, arch, type)
			 VALUES (?, ?, ?, ?, ?)`,
			&sqlitex.ExecOptions{Args: []any{pkg.ID, inst.URL, inst.Hash, inst.Arch, inst.Type}},
		)
		if err != nil {
			return fmt.Errorf("failed to insert installer for %s: %w", pkg.ID, err)
		}
	}

	return nil
}
