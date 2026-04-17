package winget

import (
	"bufio"
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"net/url"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"

	"gopkg.in/yaml.v3"

	"infra/crawler/internal/retry"
)

const (
	wingetEnvelopeSchemaVersion = 1
	wingetEnvelopeKind          = "package"
	wingetManifestRepoBaseURL   = "https://raw.githubusercontent.com/microsoft/winget-pkgs/master"
	wingetManifestCacheRoot     = "winget-manifests"
	wingetManifestMaxSize       = 1 << 20
)

type wingetManifest struct {
	PackageIdentifier   string                    `yaml:"PackageIdentifier"`
	PackageVersion      string                    `yaml:"PackageVersion"`
	PackageLocale       string                    `yaml:"PackageLocale,omitempty"`
	DefaultLocale       string                    `yaml:"DefaultLocale,omitempty"`
	Moniker             string                    `yaml:"Moniker,omitempty"`
	Tags                []string                  `yaml:"Tags,omitempty"`
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

type wingetPackageSkipSummary struct {
	count   int
	sample  []string
	example string
}

func (s *Source) WriteJSONL(ctx context.Context, dbPath string, w io.Writer, maxAttempts int, backoff time.Duration) (err error) {
	start := time.Now()
	if err := ctx.Err(); err != nil {
		return err
	}
	if strings.TrimSpace(dbPath) == "" {
		return fmt.Errorf("database path cannot be empty")
	}

	slog.Info("winget package resolution started", "db_path", dbPath, "purpose", "query the Winget index, fetch raw manifests from winget-pkgs, and write merged JSONL")

	writer, flush := bufferJSONLWriter(w)
	defer func() {
		if flushErr := flush(); err == nil && flushErr != nil {
			err = fmt.Errorf("failed to flush JSONL writer: %w", flushErr)
		}
	}()

	enc := json.NewEncoder(writer)
	indexStart := time.Now()
	rows, err := readWingetIndexRows(ctx, dbPath)
	if err != nil {
		return err
	}
	slog.Info("winget package index loaded", "db_path", dbPath, "packages", len(rows), "elapsed", time.Since(indexStart))
	if len(rows) == 0 {
		slog.Info("winget package resolution complete", "db_path", dbPath, "packages", 0, "written", 0, "skipped", 0, "elapsed", time.Since(start))
		return nil
	}

	results := make([]wingetWriteResult, len(rows))
	jobs := make(chan int)
	workerCount := 8
	if len(rows) < workerCount {
		workerCount = len(rows)
	}
	slog.Info("winget manifest fanout started", "db_path", dbPath, "packages", len(rows), "workers", workerCount)

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
				packageStart := time.Now()
				pkg, err := s.buildPackageSnapshot(ctx, row, maxAttempts, backoff)
				results[idx] = wingetWriteResult{id: row.id, pkg: pkg, err: err}
				if err != nil {
					slog.Debug("winget package crawl failed", "package", row.id, "version", row.version, "elapsed", time.Since(packageStart), "err", err)
					continue
				}

				slog.Debug("winget package crawl complete", "package", row.id, "version", row.version, "installers", len(pkg.Installers), "elapsed", time.Since(packageStart))
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

	written := 0
	skipped := 0
	skipSummaries := make(map[string]*wingetPackageSkipSummary)

	for _, result := range results {
		if result.err != nil {
			skipped++
			reason := classifyWingetPackageSkip(result.err)
			summary := skipSummaries[reason]
			if summary == nil {
				summary = &wingetPackageSkipSummary{}
				skipSummaries[reason] = summary
			}
			summary.count++
			if summary.example == "" {
				summary.example = result.err.Error()
			}
			if len(summary.sample) < 5 {
				summary.sample = append(summary.sample, result.id)
			}
			continue
		}
		written++

		if err := enc.Encode(wingetEnvelope{
			SchemaVersion: wingetEnvelopeSchemaVersion,
			Source:        sourceName,
			Kind:          wingetEnvelopeKind,
			Payload:       result.pkg,
		}); err != nil {
			return fmt.Errorf("failed to encode winget package %s: %w", result.pkg.ID, err)
		}
	}

	if skipped > 0 {
		reasons := make([]string, 0, len(skipSummaries))
		for reason := range skipSummaries {
			reasons = append(reasons, reason)
		}
		sort.Strings(reasons)
		for _, reason := range reasons {
			summary := skipSummaries[reason]
			slog.Warn("winget package skips summarized", "reason", reason, "count", summary.count, "sample_packages", summary.sample, "example_error", summary.example)
		}
	}

	slog.Info("winget package resolution complete", "db_path", dbPath, "packages", len(rows), "written", written, "skipped", skipped, "elapsed", time.Since(start))

	return nil
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
	scope, err := resolveWingetScope(firstNonEmpty(installer.Scope, defaults.Scope))
	if err != nil {
		return wingetInstallerSnapshot{}, err
	}

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
	return strings.ToLower(strings.TrimSpace(value))
}

func resolveWingetScope(value string) (string, error) {
	normalized := normalizeWingetScope(value)
	switch normalized {
	case "":
		return "", nil
	case "user", "machine":
		return normalized, nil
	default:
		return "", fmt.Errorf("unsupported winget scope %q", value)
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

func classifyWingetPackageSkip(err error) string {
	if err == nil {
		return "none"
	}

	var statusErr wingetDownloadStatusError
	if errors.As(err, &statusErr) {
		switch statusErr.StatusCode {
		case http.StatusNotFound:
			return "missing_manifest_404"
		case http.StatusTooManyRequests:
			return "download_http_429"
		default:
			if statusErr.StatusCode >= http.StatusBadRequest && statusErr.StatusCode < http.StatusInternalServerError {
				return fmt.Sprintf("download_http_%d", statusErr.StatusCode)
			}
			return fmt.Sprintf("download_status_%d", statusErr.StatusCode)
		}
	}

	message := strings.ToLower(err.Error())
	switch {
	case strings.Contains(message, "missing locale manifest"):
		return "missing_locale_manifest"
	case strings.Contains(message, "missing installer manifest"):
		return "missing_installer_manifest"
	case strings.Contains(message, "missing default locale"):
		return "missing_default_locale"
	case strings.Contains(message, "identifier mismatch"):
		return "identifier_mismatch"
	case strings.Contains(message, "unsupported winget manifest type"):
		return "unsupported_manifest_type"
	case strings.Contains(message, "has no installers"):
		return "no_installers"
	case strings.Contains(message, "unsupported winget scope"):
		return "unsupported_scope"
	default:
		return "other"
	}
}
