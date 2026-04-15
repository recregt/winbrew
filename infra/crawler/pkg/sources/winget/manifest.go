package winget

import (
	"bufio"
	"context"
	"database/sql"
	"encoding/json"
	"fmt"
	"io"
	"log/slog"
	"net/url"
	"os"
	"path"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"gopkg.in/yaml.v3"

	"infra/crawler/internal/retry"

	_ "modernc.org/sqlite"
)

const (
	wingetEnvelopeSchemaVersion = 1
	wingetEnvelopeKind          = "package"
	wingetManifestRepoBaseURL   = "https://raw.githubusercontent.com/microsoft/winget-pkgs/main"
	wingetManifestCacheRoot     = "winget-manifests"
	wingetManifestMaxSize       = 1 << 20
	wingetIndexQuery            = `
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
ORDER BY i.id ASC
`
)

type wingetIndexRow struct {
	id        string
	name      string
	version   string
	publisher string
}

type wingetManifest struct {
	PackageIdentifier   string                    `yaml:"PackageIdentifier"`
	PackageVersion      string                    `yaml:"PackageVersion"`
	PackageLocale       string                    `yaml:"PackageLocale,omitempty"`
	DefaultLocale       string                    `yaml:"DefaultLocale,omitempty"`
	Publisher           string                    `yaml:"Publisher,omitempty"`
	PackageName         string                    `yaml:"PackageName,omitempty"`
	ShortDescription    string                    `yaml:"ShortDescription,omitempty"`
	Description         string                    `yaml:"Description,omitempty"`
	Homepage            string                    `yaml:"Homepage,omitempty"`
	License             string                    `yaml:"License,omitempty"`
	ManifestType        string                    `yaml:"ManifestType"`
	ManifestVersion     string                    `yaml:"ManifestVersion"`
	Architecture        string                    `yaml:"Architecture,omitempty"`
	InstallerLocale     string                    `yaml:"InstallerLocale,omitempty"`
	Platform            string                    `yaml:"Platform,omitempty"`
	MinimumOSVersion    string                    `yaml:"MinimumOSVersion,omitempty"`
	InstallerType       string                    `yaml:"InstallerType,omitempty"`
	InstallerUrl        string                    `yaml:"InstallerUrl,omitempty"`
	InstallerSha256     string                    `yaml:"InstallerSha256,omitempty"`
	SignatureSha256     string                    `yaml:"SignatureSha256,omitempty"`
	NestedInstallerType string                    `yaml:"NestedInstallerType,omitempty"`
	Scope               string                    `yaml:"Scope,omitempty"`
	Installers          []wingetManifestInstaller `yaml:"Installers,omitempty"`
}

type wingetManifestInstaller struct {
	Architecture        string `yaml:"Architecture,omitempty"`
	InstallerLocale     string `yaml:"InstallerLocale,omitempty"`
	Platform            string `yaml:"Platform,omitempty"`
	MinimumOSVersion    string `yaml:"MinimumOSVersion,omitempty"`
	InstallerType       string `yaml:"InstallerType,omitempty"`
	InstallerUrl        string `yaml:"InstallerUrl,omitempty"`
	InstallerSha256     string `yaml:"InstallerSha256,omitempty"`
	SignatureSha256     string `yaml:"SignatureSha256,omitempty"`
	NestedInstallerType string `yaml:"NestedInstallerType,omitempty"`
	Scope               string `yaml:"Scope,omitempty"`
}

type wingetPackageSnapshot struct {
	ID          string                    `json:"id"`
	Name        string                    `json:"name"`
	Version     string                    `json:"version"`
	Description string                    `json:"description,omitempty"`
	Homepage    string                    `json:"homepage,omitempty"`
	License     string                    `json:"license,omitempty"`
	Publisher   string                    `json:"publisher,omitempty"`
	Installers  []wingetInstallerSnapshot `json:"installers,omitempty"`
}

type wingetInstallerSnapshot struct {
	URL        string `json:"url"`
	Hash       string `json:"hash,omitempty"`
	Arch       string `json:"arch,omitempty"`
	Type       string `json:"type"`
	NestedKind string `json:"NestedInstallerType,omitempty"`
	Scope      string `json:"scope,omitempty"`
}

type wingetEnvelope struct {
	SchemaVersion int                   `json:"schema_version"`
	Source        string                `json:"source"`
	Kind          string                `json:"kind"`
	Payload       wingetPackageSnapshot `json:"payload"`
}

type wingetWriteResult struct {
	id  string
	pkg wingetPackageSnapshot
	err error
}

func (s *Source) WriteJSONL(ctx context.Context, dbPath string, w io.Writer, maxAttempts int, backoff time.Duration) (err error) {
	if err := ctx.Err(); err != nil {
		return err
	}
	if strings.TrimSpace(dbPath) == "" {
		return fmt.Errorf("database path cannot be empty")
	}

	writer, flush := bufferJSONLWriter(w)
	defer func() {
		if flushErr := flush(); err == nil && flushErr != nil {
			err = fmt.Errorf("failed to flush JSONL writer: %w", flushErr)
		}
	}()

	enc := json.NewEncoder(writer)
	rows, err := readWingetIndexRows(ctx, dbPath)
	if err != nil {
		return err
	}
	if len(rows) == 0 {
		return nil
	}

	results := make([]wingetWriteResult, len(rows))
	jobs := make(chan int)
	workerCount := 8
	if len(rows) < workerCount {
		workerCount = len(rows)
	}

	var wg sync.WaitGroup
	wg.Add(workerCount)
	for i := 0; i < workerCount; i++ {
		go func() {
			defer wg.Done()
			for idx := range jobs {
				if ctxErr := ctx.Err(); ctxErr != nil {
					return
				}

				row := rows[idx]
				pkg, err := s.buildPackageSnapshot(ctx, row, maxAttempts, backoff)
				results[idx] = wingetWriteResult{id: row.id, pkg: pkg, err: err}
			}
		}()
	}

	for idx := range rows {
		select {
		case <-ctx.Done():
			close(jobs)
			wg.Wait()
			return ctx.Err()
		case jobs <- idx:
		}
	}
	close(jobs)
	wg.Wait()

	if err := ctx.Err(); err != nil {
		return err
	}

	for _, result := range results {
		if result.err != nil {
			slog.Warn("skipping winget package", "package", result.id, "err", result.err)
			continue
		}

		if err := enc.Encode(wingetEnvelope{
			SchemaVersion: wingetEnvelopeSchemaVersion,
			Source:        sourceName,
			Kind:          wingetEnvelopeKind,
			Payload:       result.pkg,
		}); err != nil {
			return fmt.Errorf("failed to encode winget package %s: %w", result.pkg.ID, err)
		}
	}

	return nil
}

func readWingetIndexRows(ctx context.Context, dbPath string) ([]wingetIndexRow, error) {
	dsn, err := sqliteDSN(dbPath)
	if err != nil {
		return nil, err
	}

	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, fmt.Errorf("failed to open winget database: %w", err)
	}
	defer db.Close()

	rows, err := db.QueryContext(ctx, wingetIndexQuery)
	if err != nil {
		return nil, fmt.Errorf("failed to query winget database: %w", err)
	}
	defer rows.Close()

	result := make([]wingetIndexRow, 0, 1024)
	for rows.Next() {
		var row wingetIndexRow
		var publisher sql.NullString
		if err := rows.Scan(&row.id, &row.name, &row.version, &publisher); err != nil {
			return nil, fmt.Errorf("failed to scan winget row: %w", err)
		}
		if publisher.Valid {
			row.publisher = strings.TrimSpace(publisher.String)
		}
		result = append(result, row)
	}

	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("failed to iterate winget rows: %w", err)
	}

	return result, nil
}

func sqliteDSN(dbPath string) (string, error) {
	absPath, err := filepath.Abs(dbPath)
	if err != nil {
		return "", fmt.Errorf("failed to resolve winget database path: %w", err)
	}

	return (&url.URL{
		Scheme:   "file",
		Path:     filepath.ToSlash(absPath),
		RawQuery: "mode=ro",
	}).String(), nil
}

func (s *Source) buildPackageSnapshot(ctx context.Context, row wingetIndexRow, maxAttempts int, backoff time.Duration) (wingetPackageSnapshot, error) {
	if row.id == "" {
		return wingetPackageSnapshot{}, fmt.Errorf("winget package id cannot be empty")
	}

	rootFile := rootNameFromIdentifier(row.id)
	rootBytes, err := s.fetchManifestBytes(ctx, row.id, row.version, rootFile, maxAttempts, backoff)
	if err != nil {
		return wingetPackageSnapshot{}, fmt.Errorf("failed to fetch winget root manifest for %s: %w", row.id, err)
	}

	rootManifest, err := parseWingetManifest(rootBytes)
	if err != nil {
		return wingetPackageSnapshot{}, fmt.Errorf("failed to parse winget root manifest for %s: %w", row.id, err)
	}

	if err := ensureWingetPackageCoordinate(row.id, rootManifest.PackageIdentifier); err != nil {
		return wingetPackageSnapshot{}, err
	}

	var localeManifest *wingetManifest
	var installerManifest *wingetManifest
	switch strings.ToLower(strings.TrimSpace(rootManifest.ManifestType)) {
	case "version":
		if strings.TrimSpace(rootManifest.DefaultLocale) == "" {
			return wingetPackageSnapshot{}, fmt.Errorf("winget package %s is missing default locale", row.id)
		}

		localeBytes, err := s.fetchManifestBytes(ctx, row.id, row.version, rootLocaleFileName(row.id, rootManifest.DefaultLocale), maxAttempts, backoff)
		if err != nil {
			return wingetPackageSnapshot{}, fmt.Errorf("failed to fetch winget locale manifest for %s: %w", row.id, err)
		}
		parsedLocale, err := parseWingetManifest(localeBytes)
		if err != nil {
			return wingetPackageSnapshot{}, fmt.Errorf("failed to parse winget locale manifest for %s: %w", row.id, err)
		}
		localeManifest = &parsedLocale

		installerBytes, err := s.fetchManifestBytes(ctx, row.id, row.version, rootInstallerFileName(row.id), maxAttempts, backoff)
		if err != nil {
			return wingetPackageSnapshot{}, fmt.Errorf("failed to fetch winget installer manifest for %s: %w", row.id, err)
		}
		parsedInstaller, err := parseWingetManifest(installerBytes)
		if err != nil {
			return wingetPackageSnapshot{}, fmt.Errorf("failed to parse winget installer manifest for %s: %w", row.id, err)
		}
		installerManifest = &parsedInstaller
	}

	return buildWingetPackageSnapshot(row, rootManifest, localeManifest, installerManifest)
}

func (s *Source) fetchManifestBytes(ctx context.Context, packageIdentifier, packageVersion, fileName string, maxAttempts int, backoff time.Duration) ([]byte, error) {
	url, err := wingetManifestURL(packageIdentifier, packageVersion, fileName)
	if err != nil {
		return nil, err
	}

	cachePath, err := wingetManifestCachePath(s.cacheDir, packageIdentifier, packageVersion, fileName)
	if err != nil {
		return nil, err
	}
	if err := os.MkdirAll(filepath.Dir(cachePath), 0o750); err != nil {
		return nil, fmt.Errorf("failed to create winget manifest cache dir: %w", err)
	}

	return retry.DoResult(ctx, maxAttempts, backoff, func() ([]byte, error) {
		if err := s.download(ctx, url, cachePath); err != nil {
			return nil, err
		}

		data, err := os.ReadFile(cachePath)
		if err != nil {
			return nil, fmt.Errorf("failed to read cached winget manifest %s: %w", cachePath, err)
		}
		if len(data) > wingetManifestMaxSize {
			return nil, fmt.Errorf("winget manifest exceeds %d bytes: %s", wingetManifestMaxSize, cachePath)
		}

		return data, nil
	})
}

func parseWingetManifest(data []byte) (wingetManifest, error) {
	if len(data) == 0 {
		return wingetManifest{}, fmt.Errorf("winget manifest cannot be empty")
	}

	var manifest wingetManifest
	if err := yaml.Unmarshal(data, &manifest); err != nil {
		return wingetManifest{}, fmt.Errorf("failed to parse winget manifest: %w", err)
	}

	return manifest, nil
}

func (m wingetManifest) resolveInstallers() ([]wingetInstallerSnapshot, error) {
	installers := m.Installers
	if len(installers) == 0 && (strings.TrimSpace(m.InstallerUrl) != "" || strings.TrimSpace(m.InstallerType) != "" || strings.TrimSpace(m.InstallerSha256) != "") {
		installers = []wingetManifestInstaller{{
			Architecture:        m.Architecture,
			InstallerLocale:     m.InstallerLocale,
			Platform:            m.Platform,
			MinimumOSVersion:    m.MinimumOSVersion,
			InstallerType:       m.InstallerType,
			InstallerUrl:        m.InstallerUrl,
			InstallerSha256:     m.InstallerSha256,
			SignatureSha256:     m.SignatureSha256,
			NestedInstallerType: m.NestedInstallerType,
			Scope:               m.Scope,
		}}
	}

	if len(installers) == 0 {
		return nil, fmt.Errorf("winget manifest %s has no installers", m.PackageIdentifier)
	}

	resolved := make([]wingetInstallerSnapshot, 0, len(installers))
	for _, installer := range installers {
		snapshot, err := installer.resolve(m)
		if err != nil {
			return nil, err
		}
		resolved = append(resolved, snapshot)
	}

	return resolved, nil
}

func (installer wingetManifestInstaller) resolve(defaults wingetManifest) (wingetInstallerSnapshot, error) {
	architecture := normalizeWingetArchitecture(firstNonEmpty(installer.Architecture, defaults.Architecture))
	installerType := normalizeWingetInstallerType(firstNonEmpty(installer.InstallerType, defaults.InstallerType))
	installerURL := strings.TrimSpace(firstNonEmpty(installer.InstallerUrl, defaults.InstallerUrl))
	installerHash := strings.TrimSpace(firstNonEmpty(installer.InstallerSha256, defaults.InstallerSha256))
	nestedInstallerType := normalizeWingetInstallerType(firstNonEmpty(installer.NestedInstallerType, defaults.NestedInstallerType))
	scope := normalizeWingetScope(firstNonEmpty(installer.Scope, defaults.Scope))

	if installerURL == "" {
		return wingetInstallerSnapshot{}, fmt.Errorf("winget installer for %s is missing InstallerUrl", defaults.PackageIdentifier)
	}
	if installerType == "" {
		return wingetInstallerSnapshot{}, fmt.Errorf("winget installer for %s is missing InstallerType", defaults.PackageIdentifier)
	}

	snapshot := wingetInstallerSnapshot{
		URL:        installerURL,
		Hash:       installerHash,
		Arch:       architecture,
		Type:       installerType,
		NestedKind: nestedInstallerType,
		Scope:      scope,
	}

	return snapshot, nil
}

func normalizeWingetArchitecture(value string) string {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "x64":
		return "x64"
	case "x86":
		return "x86"
	case "arm64", "aarch64":
		return "arm64"
	case "amd64":
		return "x64"
	case "arm", "neutral", "any", "unknown", "":
		return ""
	default:
		return ""
	}
}

func normalizeWingetInstallerType(value string) string {
	return strings.ToLower(strings.TrimSpace(value))
}

func normalizeWingetScope(value string) string {
	switch strings.ToLower(strings.TrimSpace(value)) {
	case "user", "installed":
		return "installed"
	case "machine", "provisioned":
		return "provisioned"
	default:
		return ""
	}
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if trimmed := strings.TrimSpace(value); trimmed != "" {
			return trimmed
		}
	}

	return ""
}

func ensureWingetPackageCoordinate(expected, actual string) error {
	if trimmed := strings.TrimSpace(actual); trimmed != "" && trimmed != expected {
		return fmt.Errorf("winget manifest identifier mismatch: expected %s, got %s", expected, trimmed)
	}

	return nil
}

func buildWingetPackageSnapshot(row wingetIndexRow, rootManifest wingetManifest, localeManifest, installerManifest *wingetManifest) (wingetPackageSnapshot, error) {
	if err := ensureWingetPackageCoordinate(row.id, rootManifest.PackageIdentifier); err != nil {
		return wingetPackageSnapshot{}, err
	}

	packageType := strings.ToLower(strings.TrimSpace(rootManifest.ManifestType))
	switch packageType {
	case "singleton":
		installers, err := rootManifest.resolveInstallers()
		if err != nil {
			return wingetPackageSnapshot{}, fmt.Errorf("failed to resolve winget installers for %s: %w", row.id, err)
		}

		return wingetPackageSnapshot{
			ID:          "winget/" + row.id,
			Name:        firstNonEmpty(rootManifest.PackageName, row.name),
			Version:     firstNonEmpty(rootManifest.PackageVersion, row.version),
			Description: firstNonEmpty(rootManifest.ShortDescription, rootManifest.Description),
			Homepage:    strings.TrimSpace(rootManifest.Homepage),
			License:     strings.TrimSpace(rootManifest.License),
			Publisher:   firstNonEmpty(rootManifest.Publisher, row.publisher),
			Installers:  installers,
		}, nil
	case "version":
		if localeManifest == nil {
			return wingetPackageSnapshot{}, fmt.Errorf("winget package %s is missing locale manifest", row.id)
		}
		if installerManifest == nil {
			return wingetPackageSnapshot{}, fmt.Errorf("winget package %s is missing installer manifest", row.id)
		}

		if err := ensureWingetPackageCoordinate(row.id, localeManifest.PackageIdentifier); err != nil {
			return wingetPackageSnapshot{}, err
		}
		if err := ensureWingetPackageCoordinate(row.id, installerManifest.PackageIdentifier); err != nil {
			return wingetPackageSnapshot{}, err
		}

		installers, err := installerManifest.resolveInstallers()
		if err != nil {
			return wingetPackageSnapshot{}, fmt.Errorf("failed to resolve winget installers for %s: %w", row.id, err)
		}

		return wingetPackageSnapshot{
			ID:          "winget/" + row.id,
			Name:        firstNonEmpty(localeManifest.PackageName, rootManifest.PackageName, row.name),
			Version:     firstNonEmpty(rootManifest.PackageVersion, row.version),
			Description: firstNonEmpty(localeManifest.ShortDescription, localeManifest.Description, rootManifest.ShortDescription, rootManifest.Description),
			Homepage:    firstNonEmpty(localeManifest.Homepage, rootManifest.Homepage),
			License:     firstNonEmpty(localeManifest.License, rootManifest.License),
			Publisher:   firstNonEmpty(localeManifest.Publisher, rootManifest.Publisher, row.publisher),
			Installers:  installers,
		}, nil
	default:
		return wingetPackageSnapshot{}, fmt.Errorf("unsupported winget manifest type %q for %s", rootManifest.ManifestType, row.id)
	}
}

func wingetManifestPathParts(packageIdentifier, packageVersion, fileName string) ([]string, error) {
	packageIdentifier = strings.TrimSpace(packageIdentifier)
	packageVersion = strings.TrimSpace(packageVersion)
	fileName = strings.TrimSpace(fileName)

	if packageIdentifier == "" {
		return nil, fmt.Errorf("package identifier cannot be empty")
	}
	if packageVersion == "" {
		return nil, fmt.Errorf("package version cannot be empty")
	}
	if fileName == "" {
		return nil, fmt.Errorf("manifest file name cannot be empty")
	}

	segments := strings.Split(packageIdentifier, ".")
	for _, segment := range segments {
		if strings.TrimSpace(segment) == "" {
			return nil, fmt.Errorf("package identifier contains an empty segment: %s", packageIdentifier)
		}
	}

	partition := strings.ToLower(string([]rune(segments[0])[0]))
	parts := make([]string, 0, len(segments)+3)
	parts = append(parts, "manifests", partition)
	parts = append(parts, segments...)
	parts = append(parts, packageVersion, fileName)
	return parts, nil
}

func wingetManifestURL(packageIdentifier, packageVersion, fileName string) (string, error) {
	parts, err := wingetManifestPathParts(packageIdentifier, packageVersion, fileName)
	if err != nil {
		return "", err
	}

	escaped := make([]string, 0, len(parts))
	for _, part := range parts {
		escaped = append(escaped, url.PathEscape(part))
	}

	return wingetManifestRepoBaseURL + "/" + path.Join(escaped...), nil
}

func wingetManifestCachePath(cacheDir, packageIdentifier, packageVersion, fileName string) (string, error) {
	parts, err := wingetManifestPathParts(packageIdentifier, packageVersion, fileName)
	if err != nil {
		return "", err
	}

	allParts := make([]string, 0, len(parts)+1)
	allParts = append(allParts, cacheDir, wingetManifestCacheRoot)
	allParts = append(allParts, parts...)
	return filepath.Join(allParts...), nil
}

func rootNameFromIdentifier(packageIdentifier string) string {
	return packageIdentifier + ".yaml"
}

func rootLocaleFileName(packageIdentifier, locale string) string {
	return fmt.Sprintf("%s.locale.%s.yaml", packageIdentifier, locale)
}

func rootInstallerFileName(packageIdentifier string) string {
	return fmt.Sprintf("%s.installer.yaml", packageIdentifier)
}

func bufferJSONLWriter(w io.Writer) (io.Writer, func() error) {
	if bw, ok := w.(*bufio.Writer); ok {
		return bw, bw.Flush
	}

	bw := bufio.NewWriterSize(w, 64*1024)
	return bw, bw.Flush
}
