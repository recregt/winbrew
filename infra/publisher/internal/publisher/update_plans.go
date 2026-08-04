package publisher

import (
	"fmt"
	"net/url"
	"os"
	"path"
	"path/filepath"
	"strconv"
	"strings"
)

const defaultPublicBaseURL = "https://cdn.winbrew.dev"
const latestFullRowKeyPrefix = "full:"

func WriteUpdatePlansSQL(outputPath, metadataPath, objectKey string, fullSnapshotBytes int64, patchChain []patchChainArtifact) error {
	outputPath = strings.TrimSpace(outputPath)
	if outputPath == "" {
		return fmt.Errorf("update plans output path cannot be empty")
	}

	metadata, err := LoadMetadata(metadataPath)
	if err != nil {
		return err
	}

	publicBaseURL := strings.TrimSpace(os.Getenv("CATALOG_PUBLIC_BASE_URL"))
	if publicBaseURL == "" {
		publicBaseURL = defaultPublicBaseURL
	}

	sql, err := buildUpdatePlansSQL(publicBaseURL, objectKey, metadata, fullSnapshotBytes, patchChain)
	if err != nil {
		return err
	}

	if err := os.MkdirAll(filepath.Dir(outputPath), 0o750); err != nil {
		return fmt.Errorf("failed to create update plans directory: %w", err)
	}

	return writeFileAtomic(outputPath, []byte(sql), 0o644)
}

func buildUpdatePlansSQL(publicBaseURL, objectKey string, metadata Metadata, fullSnapshotBytes int64, patchChain []patchChainArtifact) (string, error) {
	rows, err := buildUpdatePlanRows(publicBaseURL, objectKey, metadata, fullSnapshotBytes, patchChain)
	if err != nil {
		return "", err
	}

	statements := []string{
		"PRAGMA foreign_keys = ON;",
	}
	statements = append(statements, d1SchemaBootstrapStatements()...)
	statements = append(statements,
		"DELETE FROM update_plans;",
	)

	for _, row := range rows {
		statements = append(statements, row.insertStatement())
	}

	return strings.Join(statements, "\n") + "\n", nil
}

func buildUpdatePlanRows(publicBaseURL, objectKey string, metadata Metadata, fullSnapshotBytes int64, patchChain []patchChainArtifact) ([]updatePlanSQLRow, error) {
	if err := metadata.validate(); err != nil {
		return nil, err
	}
	if metadata.SchemaVersion != metadataSchemaVersion {
		return nil, fmt.Errorf("unsupported metadata schema version: %d", metadata.SchemaVersion)
	}

	snapshotURL, err := publicObjectURL(publicBaseURL, objectKey)
	if err != nil {
		return nil, err
	}

	currentHash := strings.TrimSpace(metadata.CurrentHash)
	previousHash := strings.TrimSpace(metadata.PreviousHash)

	if previousHash == "" || previousHash == currentHash {
		return []updatePlanSQLRow{{
			currentHash:     currentHash,
			mode:            "full",
			targetHash:      currentHash,
			snapshotURL:     snapshotURL,
			patchURLsJSON:   "[]",
			chainLength:     0,
			totalPatchBytes: 0,
			isLatestFull:    1,
			isStale:         0,
		}}, nil
	}

	if patchRow, ok, err := buildPatchChainRow(publicBaseURL, currentHash, previousHash, patchChain, fullSnapshotBytes); err != nil {
		return nil, err
	} else if ok {
		return []updatePlanSQLRow{
			{
				currentHash:     latestFullRowKey(currentHash),
				mode:            "full",
				targetHash:      currentHash,
				snapshotURL:     snapshotURL,
				patchURLsJSON:   "[]",
				chainLength:     0,
				totalPatchBytes: 0,
				isLatestFull:    1,
				isStale:         0,
			},
			patchRow,
			{
				currentHash:     currentHash,
				mode:            "current",
				targetHash:      currentHash,
				snapshotURL:     "",
				patchURLsJSON:   "[]",
				chainLength:     0,
				totalPatchBytes: 0,
				isLatestFull:    0,
				isStale:         0,
			},
		}, nil
	}

	return []updatePlanSQLRow{
		{
			currentHash:     previousHash,
			mode:            "full",
			targetHash:      currentHash,
			snapshotURL:     snapshotURL,
			patchURLsJSON:   "[]",
			chainLength:     0,
			totalPatchBytes: 0,
			isLatestFull:    1,
			isStale:         0,
		},
		{
			currentHash:     currentHash,
			mode:            "current",
			targetHash:      currentHash,
			snapshotURL:     "",
			patchURLsJSON:   "[]",
			chainLength:     0,
			totalPatchBytes: 0,
			isLatestFull:    0,
			isStale:         0,
		},
	}, nil
}

type updatePlanSQLRow struct {
	currentHash     string
	mode            string
	targetHash      string
	snapshotURL     string
	patchURLsJSON   string
	chainLength     int
	totalPatchBytes int64
	isLatestFull    int
	isStale         int
}

func (row updatePlanSQLRow) insertStatement() string {
	return sqlSprintf(
		"INSERT INTO update_plans (current_hash, mode, target_hash, snapshot_url, patch_urls_json, chain_length, total_patch_bytes, is_latest_full, is_stale) VALUES (%s, %s, %s, %s, %s, %s, %s, %s, %s);",
		sqlText(row.currentHash),
		sqlText(row.mode),
		sqlText(row.targetHash),
		sqlNullableText(row.snapshotURL),
		sqlText(row.patchURLsJSON),
		sqlInt(int64(row.chainLength)),
		sqlInt(row.totalPatchBytes),
		sqlInt(int64(row.isLatestFull)),
		sqlInt(int64(row.isStale)),
	)
}

func publicObjectURL(baseURL, objectKey string) (string, error) {
	baseURL = strings.TrimSpace(baseURL)
	objectKey = strings.TrimSpace(objectKey)

	if baseURL == "" {
		return "", fmt.Errorf("CATALOG_PUBLIC_BASE_URL cannot be empty")
	}
	if objectKey == "" {
		return "", fmt.Errorf("object key cannot be empty")
	}

	parsed, err := url.Parse(baseURL)
	if err != nil {
		return "", fmt.Errorf("invalid CATALOG_PUBLIC_BASE_URL: %w", err)
	}
	if parsed.Scheme != "http" && parsed.Scheme != "https" {
		return "", fmt.Errorf("unsupported CATALOG_PUBLIC_BASE_URL scheme: %s", parsed.Scheme)
	}
	if parsed.Host == "" {
		return "", fmt.Errorf("invalid CATALOG_PUBLIC_BASE_URL: %q", baseURL)
	}

	parsed.Path = path.Join(parsed.Path, strings.TrimLeft(objectKey, "/"))
	parsed.RawQuery = ""
	parsed.Fragment = ""

	return parsed.String(), nil
}

// sqlLiteral is a value that has already been rendered as safe SQL text
// (quoted-and-escaped, or a bare numeric/NULL literal). It is only ever
// constructed by sqlText, sqlNullableText, or sqlInt in this file, so it
// exists to make sqlSprintf's argument list type-checked: a raw string or
// int passed where a sqlLiteral is expected fails to compile instead of
// shipping an unescaped value into a SQL statement that gets executed
// directly against production D1 via `wrangler d1 execute`.
type sqlLiteral string

func (v sqlLiteral) renderSQL() string { return string(v) }

// sqlArg is implemented only by sqlLiteral, so sqlSprintf's format
// arguments must already be escaped/rendered safe SQL text.
type sqlArg interface {
	renderSQL() string
}

// sqlSprintf is fmt.Sprintf restricted to sqlArg values. Use it (instead of
// fmt.Sprintf directly) anywhere a SQL statement is being assembled by
// string interpolation, so every argument is forced through sqlText,
// sqlNullableText, or sqlInt first.
func sqlSprintf(format string, args ...sqlArg) string {
	rendered := make([]any, len(args))
	for i, arg := range args {
		rendered[i] = arg.renderSQL()
	}

	return fmt.Sprintf(format, rendered...)
}

func sqlText(value string) sqlLiteral {
	return sqlLiteral("'" + strings.ReplaceAll(value, "'", "''") + "'")
}

func sqlNullableText(value string) sqlLiteral {
	if strings.TrimSpace(value) == "" {
		return "NULL"
	}

	return sqlText(value)
}

// sqlInt renders an integer as a bare (unquoted) SQL numeric literal.
func sqlInt(value int64) sqlLiteral {
	return sqlLiteral(strconv.FormatInt(value, 10))
}

func latestFullRowKey(currentHash string) string {
	return latestFullRowKeyPrefix + strings.TrimSpace(currentHash)
}
