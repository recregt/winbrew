package winget

import (
	"bytes"
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"net/http"
	"net/http/httptest"
	"net/url"
	"strings"
	"testing"
	"time"
)

func TestWingetStagingCountsWrittenAndSkippedPackages(t *testing.T) {
	db := openWingetIndexFixture(t)
	defer db.Close()

	rows, err := collectWingetIndexRows(context.Background(), db)
	if err != nil {
		t.Fatalf("collectWingetIndexRows() error = %v", err)
	}
	if got, want := len(rows), 2; got != want {
		t.Fatalf("input package count = %d, want %d", got, want)
	}

	successManifest := `
PackageIdentifier: Contoso.App
PackageVersion: 1.0.0
PackageLocale: en-US
PackageName: Contoso App
Publisher: Contoso Ltd.
Moniker: contoso
Tags:
  - utility
ShortDescription: Contoso app
Homepage: https://contoso.example
License: MIT
Installers:
  - Architecture: x64
    InstallerType: exe
    InstallerUrl: https://download.contoso.invalid/app.exe
    InstallerSha256: 0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF
    Scope: machine
ManifestType: singleton
ManifestVersion: 1.12.0
`
	server := httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		switch {
		case strings.HasSuffix(r.URL.Path, "/manifests/c/Contoso/App/1.0.0/Contoso.App.yaml"):
			w.WriteHeader(http.StatusOK)
			_, _ = fmt.Fprint(w, successManifest)
		case strings.HasSuffix(r.URL.Path, "/manifests/m/Missing/App/3.0.0/Missing.App.yaml"):
			w.WriteHeader(http.StatusNotFound)
		default:
			w.WriteHeader(http.StatusNotFound)
		}
	}))
	defer server.Close()

	targetURL, err := url.Parse(server.URL)
	if err != nil {
		t.Fatalf("url.Parse() error = %v", err)
	}

	client := &http.Client{Transport: rewritingTransport{target: targetURL, base: http.DefaultTransport}}
	src, err := New(client, t.TempDir())
	if err != nil {
		t.Fatalf("New() error = %v", err)
	}

	written := make([]wingetEnvelope, 0, 1)
	skipped := 0
	for _, row := range rows {
		pkg, err := src.buildPackageSnapshot(context.Background(), row, 1, time.Millisecond)
		if err != nil {
			if row.id == "Contoso.App" {
				t.Fatalf("buildPackageSnapshot(%s) error = %v", row.id, err)
			}
			skipped++
			continue
		}

		written = append(written, wingetEnvelope{
			SchemaVersion: wingetEnvelopeSchemaVersion,
			Source:        sourceName,
			Kind:          wingetEnvelopeKind,
			Payload:       pkg,
		})
	}

	if got, want := skipped, 1; got != want {
		t.Fatalf("skipped package count = %d, want %d", got, want)
	}
	if got, want := len(written), 1; got != want {
		t.Fatalf("written package count = %d, want %d", got, want)
	}

	var output bytes.Buffer
	enc := json.NewEncoder(&output)
	for _, envelope := range written {
		if err := enc.Encode(envelope); err != nil {
			t.Fatalf("json.Encoder.Encode() error = %v", err)
		}
	}

	rawOutput := strings.TrimSpace(output.String())
	if rawOutput == "" {
		t.Fatal("staged JSONL produced no output")
	}

	lines := strings.Split(rawOutput, "\n")
	if got, want := len(lines), 1; got != want {
		t.Fatalf("merged JSONL line count = %d, want %d", got, want)
	}

	var envelope wingetEnvelope
	if err := json.Unmarshal([]byte(lines[0]), &envelope); err != nil {
		t.Fatalf("json.Unmarshal() error = %v", err)
	}
	if got, want := envelope.Source, sourceName; got != want {
		t.Fatalf("envelope source = %q, want %q", got, want)
	}
	if got, want := envelope.Payload.ID, "winget/Contoso.App"; got != want {
		t.Fatalf("envelope package id = %q, want %q", got, want)
	}
	if got, want := len(envelope.Payload.Installers), 1; got != want {
		t.Fatalf("merged installer count = %d, want %d", got, want)
	}
}

func openWingetIndexFixture(t *testing.T) *sql.DB {
	t.Helper()

	db, err := sql.Open("sqlite", "file:winget-staging-counts?mode=memory&cache=shared")
	if err != nil {
		t.Fatalf("sql.Open() error = %v", err)
	}

	statements := []string{
		`CREATE TABLE ids (id TEXT NOT NULL);`,
		`CREATE TABLE names (name TEXT NOT NULL);`,
		`CREATE TABLE versions (version TEXT NOT NULL);`,
		`CREATE TABLE manifest (id INTEGER NOT NULL, name INTEGER NOT NULL, version INTEGER NOT NULL);`,
		`CREATE TABLE norm_publishers (norm_publisher TEXT NOT NULL);`,
		`CREATE TABLE norm_publishers_map (manifest INTEGER NOT NULL, norm_publisher INTEGER NOT NULL);`,
		`INSERT INTO ids (rowid, id) VALUES (1, 'Contoso.App');`,
		`INSERT INTO names (rowid, name) VALUES (1, 'Contoso App');`,
		`INSERT INTO versions (rowid, version) VALUES (1, '1.0.0');`,
		`INSERT INTO ids (rowid, id) VALUES (2, 'Missing.App');`,
		`INSERT INTO names (rowid, name) VALUES (2, 'Missing App');`,
		`INSERT INTO versions (rowid, version) VALUES (2, '3.0.0');`,
		`INSERT INTO norm_publishers (rowid, norm_publisher) VALUES (1, 'Contoso Ltd.');`,
		`INSERT INTO norm_publishers_map (manifest, norm_publisher) VALUES (1, 1);`,
		`INSERT INTO manifest (rowid, id, name, version) VALUES (1, 1, 1, 1);`,
		`INSERT INTO manifest (rowid, id, name, version) VALUES (2, 2, 2, 2);`,
	}

	for _, statement := range statements {
		if _, err := db.Exec(statement); err != nil {
			t.Fatalf("db.Exec(%q) error = %v", statement, err)
		}
	}

	return db
}

type rewritingTransport struct {
	target *url.URL
	base   http.RoundTripper
}

func (rt rewritingTransport) RoundTrip(req *http.Request) (*http.Response, error) {
	clone := req.Clone(req.Context())
	clone.URL.Scheme = rt.target.Scheme
	clone.URL.Host = rt.target.Host
	clone.Host = rt.target.Host
	clone.URL.Path = req.URL.Path
	clone.URL.RawPath = req.URL.RawPath
	clone.URL.RawQuery = req.URL.RawQuery
	if clone.URL.Scheme == "" {
		clone.URL.Scheme = "http"
	}

	base := rt.base
	if base == nil {
		base = http.DefaultTransport
	}

	return base.RoundTrip(clone)
}
