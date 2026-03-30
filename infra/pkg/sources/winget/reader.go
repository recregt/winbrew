package winget

import (
	"context"
	"encoding/json"
	"fmt"
	"strings"

	"zombiezen.com/go/sqlite"
	"zombiezen.com/go/sqlite/sqlitex"

	"winbrew/infra/pkg/normalize"
)

const query = `
SELECT
	i.id,
	n.name,
	v.version,
	np.norm_publisher
FROM manifest m
JOIN ids i        ON i.rowid = m.id
JOIN names n      ON n.rowid = m.name
JOIN versions v   ON v.rowid = m.version
LEFT JOIN norm_publishers_map npm ON npm.manifest = m.rowid
LEFT JOIN norm_publishers np      ON np.rowid = npm.norm_publisher
GROUP BY i.id
HAVING v.version = MAX(v.version)
`

func readPackages(ctx context.Context, dbPath string) ([]normalize.Package, error) {
	conn, err := sqlite.OpenConn(dbPath, sqlite.OpenReadOnly)
	if err != nil {
		return nil, fmt.Errorf("failed to open winget db: %w", err)
	}
	defer conn.Close()

	var pkgs []normalize.Package

	err = sqlitex.ExecuteTransient(conn, query, &sqlitex.ExecOptions{
		ResultFunc: func(stmt *sqlite.Stmt) error {
			if err := ctx.Err(); err != nil {
				return err
			}

			id := stmt.ColumnText(0)
			name := stmt.ColumnText(1)
			version := stmt.ColumnText(2)
			publisher := stmt.ColumnText(3)

			if id == "" {
				return fmt.Errorf("winget package row missing id")
			}

			raw, err := json.Marshal(map[string]string{
				"id":        id,
				"name":      name,
				"version":   version,
				"publisher": publisher,
			})
			if err != nil {
				return fmt.Errorf("failed to marshal raw for %s: %w", id, err)
			}

			pkgs = append(pkgs, normalize.Package{
				ID:        "winget/" + id,
				Name:      name,
				Version:   version,
				Source:    sourceName,
				Publisher: strings.TrimSpace(publisher),
				Raw:       raw,
			})
			return nil
		},
	})

	if err != nil {
		return nil, fmt.Errorf("failed to query winget db: %w", err)
	}

	return pkgs, nil
}
