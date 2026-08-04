package winget

import (
	"context"
	"database/sql"
	"testing"
)

func TestCompareWingetVersions(t *testing.T) {
	t.Parallel()

	tests := []struct {
		name  string
		left  string
		right string
		want  int
	}{
		{name: "four-part numeric version", left: "115.0.5790.9", right: "115.0.5790.136", want: -1},
		{name: "double digit segment", left: "2.9.0", right: "2.10.0", want: -1},
		{name: "revision after release", left: "1.0.0", right: "1.0.0.1", want: -1},
		{name: "prefix v", left: "v2026.03.17", right: "2026.03.16", want: 1},
		{name: "prerelease before release", left: "1.0.0-alpha", right: "1.0.0", want: -1},
	}

	for _, testCase := range tests {
		testCase := testCase
		t.Run(testCase.name, func(t *testing.T) {
			t.Parallel()

			if got := compareWingetVersions(testCase.left, testCase.right); got != testCase.want {
				t.Fatalf("compareWingetVersions(%q, %q) = %d, want %d", testCase.left, testCase.right, got, testCase.want)
			}
			if got := compareWingetVersions(testCase.right, testCase.left); got != -testCase.want {
				t.Fatalf("compareWingetVersions(%q, %q) = %d, want %d", testCase.right, testCase.left, got, -testCase.want)
			}
		})
	}
}

func TestReadWingetIndexRowsPrefersLatestVersion(t *testing.T) {
	t.Parallel()

	db, err := sql.Open("sqlite", "file:winget-index-regression?mode=memory&cache=shared")
	if err != nil {
		t.Fatalf("sql.Open() error = %v", err)
	}

	createStatements := []string{
		`CREATE TABLE ids (id TEXT NOT NULL);`,
		`CREATE TABLE names (name TEXT NOT NULL);`,
		`CREATE TABLE versions (version TEXT NOT NULL);`,
		`CREATE TABLE manifest (id INTEGER NOT NULL, name INTEGER NOT NULL, version INTEGER NOT NULL);`,
		`CREATE TABLE norm_publishers (norm_publisher TEXT NOT NULL);`,
		`CREATE TABLE norm_publishers_map (manifest INTEGER NOT NULL, norm_publisher INTEGER NOT NULL);`,
	}

	insertStatements := []string{
		`INSERT INTO ids (rowid, id) VALUES (1, 'Contoso.App');`,
		`INSERT INTO names (rowid, name) VALUES (1, 'Contoso App');`,
		`INSERT INTO versions (rowid, version) VALUES (1, '115.0.5790.9');`,
		`INSERT INTO versions (rowid, version) VALUES (2, '115.0.5790.136');`,
		`INSERT INTO norm_publishers (rowid, norm_publisher) VALUES (1, 'Contoso Ltd.');`,
		`INSERT INTO manifest (rowid, id, name, version) VALUES (1, 1, 1, 1);`,
		`INSERT INTO manifest (rowid, id, name, version) VALUES (2, 1, 1, 2);`,
		`INSERT INTO norm_publishers_map (manifest, norm_publisher) VALUES (1, 1);`,
		`INSERT INTO norm_publishers_map (manifest, norm_publisher) VALUES (2, 1);`,
	}

	for _, statement := range createStatements {
		if _, err := db.Exec(statement); err != nil {
			_ = db.Close()
			t.Fatalf("exec create statement %q: %v", statement, err)
		}
	}
	for _, statement := range insertStatements {
		if _, err := db.Exec(statement); err != nil {
			_ = db.Close()
			t.Fatalf("exec insert statement %q: %v", statement, err)
		}
	}
	rows, err := collectWingetIndexRows(context.Background(), db)
	if err != nil {
		_ = db.Close()
		t.Fatalf("collectWingetIndexRows() error = %v", err)
	}
	if err := db.Close(); err != nil {
		t.Fatalf("db.Close() error = %v", err)
	}

	if got, want := len(rows), 1; got != want {
		t.Fatalf("row count = %d, want %d", got, want)
	}
	if got, want := rows[0].id, "Contoso.App"; got != want {
		t.Fatalf("row id = %q, want %q", got, want)
	}
	if got, want := rows[0].version, "115.0.5790.136"; got != want {
		t.Fatalf("row version = %q, want %q", got, want)
	}
	if got, want := rows[0].manifestRowID, int64(2); got != want {
		t.Fatalf("row manifest rowid = %d, want %d", got, want)
	}
}
