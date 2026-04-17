package winget

import (
	"context"
	"database/sql"
	"fmt"
	"log/slog"
	"net/url"
	"path/filepath"
	"sort"
	"strings"
	"time"

	_ "modernc.org/sqlite"
)

const wingetIndexQuery = `
SELECT
    i.id,
    n.name,
    v.version,
    np.norm_publisher,
    m.rowid
FROM manifest m
JOIN ids i        ON i.rowid = m.id
JOIN names n      ON n.rowid = m.name
JOIN versions v   ON v.rowid = m.version
LEFT JOIN norm_publishers_map npm ON npm.manifest = m.rowid
LEFT JOIN norm_publishers np      ON np.rowid = npm.norm_publisher
`

type wingetIndexRow struct {
	id            string
	name          string
	version       string
	publisher     string
	manifestRowID int64
}

type wingetVersionComponent struct {
	numeric bool
	value   string
}

type wingetVersionParts struct {
	core         [3]string
	extra        []wingetVersionComponent
	hasTextExtra bool
}

func readWingetIndexRows(ctx context.Context, dbPath string) ([]wingetIndexRow, error) {
	start := time.Now()
	dsn, err := sqliteDSN(dbPath)
	if err != nil {
		return nil, err
	}

	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, fmt.Errorf("failed to open winget database: %w", err)
	}
	defer db.Close()

	result, err := collectWingetIndexRows(ctx, db)
	if err != nil {
		return nil, err
	}
	sort.Slice(result, func(i, j int) bool {
		return result[i].id < result[j].id
	})

	slog.Debug("winget index query finished", "db_path", dbPath, "packages", len(result), "elapsed", time.Since(start))

	return result, nil
}

func collectWingetIndexRows(ctx context.Context, db *sql.DB) ([]wingetIndexRow, error) {
	rows, err := db.QueryContext(ctx, wingetIndexQuery)
	if err != nil {
		return nil, fmt.Errorf("failed to query winget database: %w", err)
	}
	defer rows.Close()

	bestRows := make(map[string]wingetIndexRow, 1024)
	for rows.Next() {
		var row wingetIndexRow
		var publisher sql.NullString
		if err := rows.Scan(&row.id, &row.name, &row.version, &publisher, &row.manifestRowID); err != nil {
			return nil, fmt.Errorf("failed to scan winget row: %w", err)
		}
		if publisher.Valid {
			row.publisher = strings.TrimSpace(publisher.String)
		}

		current, exists := bestRows[row.id]
		if !exists {
			bestRows[row.id] = row
			continue
		}

		cmp := compareWingetVersions(row.version, current.version)
		if cmp > 0 || (cmp == 0 && row.manifestRowID > current.manifestRowID) {
			bestRows[row.id] = row
		}
	}

	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("failed to iterate winget rows: %w", err)
	}

	result := make([]wingetIndexRow, 0, len(bestRows))
	for _, row := range bestRows {
		result = append(result, row)
	}

	return result, nil
}

func compareWingetVersions(left, right string) int {
	leftParts := parseWingetVersion(left)
	rightParts := parseWingetVersion(right)

	for i := range leftParts.core {
		if cmp := compareWingetNumericIdentifiers(leftParts.core[i], rightParts.core[i]); cmp != 0 {
			return cmp
		}
	}

	if len(leftParts.extra) == 0 && len(rightParts.extra) == 0 {
		return 0
	}

	if len(leftParts.extra) == 0 {
		if rightParts.hasTextExtra {
			return 1
		}
		return -1
	}
	if len(rightParts.extra) == 0 {
		if leftParts.hasTextExtra {
			return -1
		}
		return 1
	}

	if !leftParts.hasTextExtra && !rightParts.hasTextExtra {
		return compareWingetNumericExtras(leftParts.extra, rightParts.extra)
	}

	if !leftParts.hasTextExtra {
		return 1
	}
	if !rightParts.hasTextExtra {
		return -1
	}

	return compareWingetTextExtras(leftParts.extra, rightParts.extra)
}

func parseWingetVersion(value string) wingetVersionParts {
	parts := wingetVersionParts{}
	trimmed := strings.TrimSpace(value)
	if trimmed == "" {
		parts.core = [3]string{"0", "0", "0"}
		return parts
	}

	trimmed = stripVersionPrefix(trimmed)
	tokens := tokenizeVersion(trimmed)
	coreIndex := 0

	for _, token := range tokens {
		if token == "" {
			continue
		}

		if coreIndex < len(parts.core) {
			if digits, suffix := splitNumericPrefix(token); digits != "" {
				parts.core[coreIndex] = normalizeNumericIdentifier(digits)
				coreIndex++
				if suffix != "" {
					parts.extra = append(parts.extra, wingetVersionComponent{value: strings.ToLower(suffix)})
					parts.hasTextExtra = true
				}
				continue
			}

			if isAllDigits(token) {
				parts.core[coreIndex] = normalizeNumericIdentifier(token)
				coreIndex++
				continue
			}

			parts.extra = append(parts.extra, wingetVersionComponent{value: strings.ToLower(token)})
			parts.hasTextExtra = true
			continue
		}

		if isAllDigits(token) {
			parts.extra = append(parts.extra, wingetVersionComponent{numeric: true, value: normalizeNumericIdentifier(token)})
		} else {
			parts.extra = append(parts.extra, wingetVersionComponent{value: strings.ToLower(token)})
			parts.hasTextExtra = true
		}
	}

	for coreIndex < len(parts.core) {
		parts.core[coreIndex] = "0"
		coreIndex++
	}

	return parts
}

func compareWingetNumericExtras(left, right []wingetVersionComponent) int {
	for idx := 0; idx < len(left) && idx < len(right); idx++ {
		if cmp := compareWingetNumericIdentifiers(left[idx].value, right[idx].value); cmp != 0 {
			return cmp
		}
	}

	switch {
	case len(left) == len(right):
		return 0
	case len(left) < len(right):
		return -1
	default:
		return 1
	}
}

func compareWingetTextExtras(left, right []wingetVersionComponent) int {
	for idx := 0; idx < len(left) && idx < len(right); idx++ {
		leftComponent := left[idx]
		rightComponent := right[idx]

		if leftComponent.numeric && rightComponent.numeric {
			if cmp := compareWingetNumericIdentifiers(leftComponent.value, rightComponent.value); cmp != 0 {
				return cmp
			}
			continue
		}

		if leftComponent.numeric != rightComponent.numeric {
			if leftComponent.numeric {
				return -1
			}
			return 1
		}

		if cmp := strings.Compare(leftComponent.value, rightComponent.value); cmp != 0 {
			return cmp
		}
	}

	switch {
	case len(left) == len(right):
		return 0
	case len(left) < len(right):
		return -1
	default:
		return 1
	}
}

func compareWingetNumericIdentifiers(left, right string) int {
	left = normalizeNumericIdentifier(left)
	right = normalizeNumericIdentifier(right)

	if len(left) != len(right) {
		if len(left) < len(right) {
			return -1
		}
		return 1
	}

	return strings.Compare(left, right)
}

func normalizeNumericIdentifier(value string) string {
	trimmed := strings.TrimLeft(value, "0")
	if trimmed == "" {
		return "0"
	}

	return trimmed
}

func isAllDigits(value string) bool {
	if value == "" {
		return false
	}

	for i := 0; i < len(value); i++ {
		if value[i] < '0' || value[i] > '9' {
			return false
		}
	}

	return true
}

func tokenizeVersion(value string) []string {
	return strings.FieldsFunc(value, func(ch rune) bool {
		return (ch < '0' || ch > '9') && (ch < 'A' || ch > 'Z') && (ch < 'a' || ch > 'z')
	})
}

func stripVersionPrefix(value string) string {
	if len(value) < 2 {
		return value
	}

	first := value[0]
	if first != 'v' && first != 'V' {
		return value
	}

	if next := value[1]; next >= '0' && next <= '9' {
		return value[1:]
	}

	return value
}

func splitNumericPrefix(value string) (string, string) {
	idx := 0
	for idx < len(value) {
		ch := value[idx]
		if ch < '0' || ch > '9' {
			break
		}
		idx++
	}

	if idx == 0 {
		return "", value
	}

	return value[:idx], value[idx:]
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
