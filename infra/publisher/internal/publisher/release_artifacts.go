package publisher

import (
	"context"
	"database/sql"
	"fmt"
	"net/url"
	"os"
	"path"
	"path/filepath"
	"sort"
	"strings"

	"github.com/minio/minio-go/v7"
	_ "modernc.org/sqlite"
)

type packageRecord struct {
	RowID       int64
	ID          string
	Name        string
	Version     string
	Source      string
	Namespace   sql.NullString
	SourceID    string
	Description sql.NullString
	Homepage    sql.NullString
	License     sql.NullString
	Publisher   sql.NullString
	Locale      sql.NullString
	Moniker     sql.NullString
	Tags        sql.NullString
	Bin         sql.NullString
	CreatedAt   string
	UpdatedAt   string
}

type rawRecord struct {
	PackageID string
	Raw       sql.NullString
}

type installerRecord struct {
	ID                int64
	PackageID         string
	URL               string
	Hash              sql.NullString
	HashAlgorithm     string
	InstallerType     string
	InstallerSwitches sql.NullString
	Scope             sql.NullString
	Arch              string
	Kind              string
	NestedKind        sql.NullString
}

type patchArtifactBundle struct {
	Artifact  patchChainArtifact
	TempPath  string
	ObjectKey string
}

func buildReleaseMaterializationSQL(publicBaseURL, objectKey string, metadata Metadata, patchArtifacts []patchChainArtifact) (string, error) {
	if err := metadata.validate(); err != nil {
		return "", err
	}
	if metadata.SchemaVersion != metadataSchemaVersion {
		return "", fmt.Errorf("unsupported metadata schema version: %d", metadata.SchemaVersion)
	}

	snapshotURL, err := publicObjectURL(publicBaseURL, objectKey)
	if err != nil {
		return "", err
	}
	metadataURL, err := publicObjectURL(publicBaseURL, metadataKeyForObjectKey(objectKey))
	if err != nil {
		return "", err
	}

	currentHash := strings.TrimSpace(metadata.CurrentHash)
	previousHash := strings.TrimSpace(metadata.PreviousHash)

	statements := []string{
		"PRAGMA foreign_keys = ON;",
	}
	statements = append(statements, d1SchemaBootstrapStatements()...)
	statements = append(statements,
		fmt.Sprintf(
			"INSERT INTO release_lineage (hash, parent_hash, is_snapshot, snapshot_url, metadata_url) VALUES (%s, %s, 1, %s, %s) ON CONFLICT(hash) DO UPDATE SET parent_hash = excluded.parent_hash, is_snapshot = excluded.is_snapshot, snapshot_url = excluded.snapshot_url, metadata_url = excluded.metadata_url;",
			sqlText(currentHash),
			sqlNullableText(previousHash),
			sqlText(snapshotURL),
			sqlText(metadataURL),
		),
	)

	for _, artifact := range patchArtifacts {
		if strings.TrimSpace(artifact.FromHash) == "" || strings.TrimSpace(artifact.ToHash) == "" || strings.TrimSpace(artifact.FilePath) == "" || strings.TrimSpace(artifact.Checksum) == "" {
			continue
		}

		statements = append(statements,
			fmt.Sprintf(
				"INSERT INTO patch_artifacts (from_hash, to_hash, file_path, size_bytes, checksum) VALUES (%s, %s, %s, %d, %s) ON CONFLICT(from_hash, to_hash) DO UPDATE SET file_path = excluded.file_path, size_bytes = excluded.size_bytes, checksum = excluded.checksum;",
				sqlText(artifact.FromHash),
				sqlText(artifact.ToHash),
				sqlText(artifact.FilePath),
				artifact.SizeBytes,
				sqlText(artifact.Checksum),
			),
		)
	}

	return strings.Join(statements, "\n") + "\n", nil
}

func writeReleaseMaterialization(path string, publicBaseURL, objectKey string, metadata Metadata, patchArtifacts []patchChainArtifact) error {
	path = strings.TrimSpace(path)
	if path == "" {
		return nil
	}

	sqlText, err := buildReleaseMaterializationSQL(publicBaseURL, objectKey, metadata, patchArtifacts)
	if err != nil {
		return err
	}

	if err := os.MkdirAll(filepath.Dir(path), 0o750); err != nil {
		return fmt.Errorf("failed to create release materialization directory: %w", err)
	}

	return writeFileAtomic(path, []byte(sqlText), 0o644)
}

func buildPatchArtifactCandidate(ctx context.Context, client *minio.Client, bucketName, publicBaseURL, objectKey, currentPath string, metadata Metadata, remoteMetadata *Metadata, fullSnapshotBytes int64) (*patchArtifactBundle, error) {
	if remoteMetadata == nil {
		return nil, nil
	}

	previousHash := strings.TrimSpace(remoteMetadata.CurrentHash)
	currentHash := strings.TrimSpace(metadata.CurrentHash)
	if previousHash == "" || currentHash == "" || previousHash == currentHash {
		return nil, nil
	}

	previousCompressedPath, err := downloadRemoteObjectToTemp(ctx, client, bucketName, objectKey)
	if err != nil {
		return nil, err
	}
	defer func() {
		_ = os.Remove(previousCompressedPath)
	}()

	previousSnapshotPath, err := decompressSnapshotToTemp(previousCompressedPath)
	if err != nil {
		return nil, err
	}
	defer func() {
		_ = os.Remove(previousSnapshotPath)
	}()

	patchSQL, err := buildCatalogPatchSQL(previousSnapshotPath, currentPath)
	if err != nil {
		return nil, err
	}

	objectKey = patchObjectKeyForRelease(previousHash, currentHash)
	patchTempPath, patchSize, err := compressTextToTemp(patchSQL, filepath.Base(objectKey)+".*.sql.zst")
	if err != nil {
		return nil, err
	}

	artifact := patchChainArtifact{
		FromHash:        previousHash,
		ToHash:          currentHash,
		Depth:           0,
		FilePath:        objectKey,
		SizeBytes:       patchSize,
		Checksum:        "",
		ReachedPrevious: true,
	}
	checksum, err := hashFile(patchTempPath)
	if err != nil {
		_ = os.Remove(patchTempPath)
		return nil, err
	}
	artifact.Checksum = checksum

	if _, ok, err := buildPatchChainRow(publicBaseURL, currentHash, previousHash, []patchChainArtifact{artifact}, fullSnapshotBytes); err != nil {
		_ = os.Remove(patchTempPath)
		return nil, err
	} else if !ok {
		_ = os.Remove(patchTempPath)
		return nil, nil
	}

	return &patchArtifactBundle{
		Artifact:  artifact,
		TempPath:  patchTempPath,
		ObjectKey: objectKey,
	}, nil
}

func patchObjectKeyForRelease(previousHash, currentHash string) string {
	return path.Join("patches", sanitizeHashComponent(previousHash)+"-"+sanitizeHashComponent(currentHash)+".sql.zst")
}

func sanitizeHashComponent(hash string) string {
	hash = strings.TrimSpace(hash)
	hash = strings.TrimPrefix(hash, "sha256:")
	if len(hash) > 16 {
		hash = hash[:16]
	}
	if hash == "" {
		return "unknown"
	}

	return hash
}

func downloadRemoteObjectToTemp(ctx context.Context, client *minio.Client, bucketName, objectKey string) (string, error) {
	tempFile, err := os.CreateTemp("", filepath.Base(objectKey)+".*.download")
	if err != nil {
		return "", fmt.Errorf("failed to create remote object temp file: %w", err)
	}
	tempPath := tempFile.Name()
	committed := false
	defer func() {
		if !committed {
			_ = tempFile.Close()
			_ = os.Remove(tempPath)
		}
	}()
	if err := tempFile.Close(); err != nil {
		return "", fmt.Errorf("failed to close remote object temp file: %w", err)
	}

	if err := client.FGetObject(ctx, bucketName, objectKey, tempPath, minio.GetObjectOptions{}); err != nil {
		return "", fmt.Errorf("failed to download remote snapshot object %s: %w", objectKey, err)
	}

	committed = true
	return tempPath, nil
}

func buildCatalogPatchSQL(previousPath, currentPath string) (string, error) {
	previousDB, err := openSQLiteReadOnly(previousPath)
	if err != nil {
		return "", err
	}
	defer previousDB.Close()

	currentDB, err := openSQLiteReadOnly(currentPath)
	if err != nil {
		return "", err
	}
	defer currentDB.Close()

	return buildCatalogPatchSQLFromDB(previousDB, currentDB)
}

func buildCatalogPatchSQLFromDB(previousDB, currentDB *sql.DB) (string, error) {

	previousPackages, previousRaws, previousInstallers, err := loadCatalogSnapshot(previousDB)
	if err != nil {
		return "", err
	}
	currentPackages, currentRaws, currentInstallers, err := loadCatalogSnapshot(currentDB)
	if err != nil {
		return "", err
	}

	changedPackages := make(map[string]struct{})
	for id, currentPackage := range currentPackages {
		previousPackage, ok := previousPackages[id]
		if !ok || !packageRecordsEqual(currentPackage, previousPackage) {
			changedPackages[id] = struct{}{}
		}
	}

	removedPackages := make([]string, 0)
	for id := range previousPackages {
		if _, ok := currentPackages[id]; !ok {
			removedPackages = append(removedPackages, id)
		}
	}
	sort.Strings(removedPackages)

	var statements []string
	statements = append(statements, "PRAGMA foreign_keys = ON;", "BEGIN;")

	packageIDs := make([]string, 0, len(currentPackages))
	for id := range currentPackages {
		packageIDs = append(packageIDs, id)
	}
	sort.Strings(packageIDs)

	for _, packageID := range packageIDs {
		currentPackage := currentPackages[packageID]
		_, packageChanged := changedPackages[packageID]

		if packageChanged {
			statements = append(statements, packageUpsertStatement(currentPackage))
			if raw, ok := currentRaws[packageID]; ok {
				statements = append(statements, rawUpsertStatement(packageID, raw))
			}

			currentPackageInstallers := currentInstallers[packageID]
			for _, installer := range currentPackageInstallers {
				statements = append(statements, installerUpsertStatement(installer))
			}

			continue
		}

		if currentRaw, ok := currentRaws[packageID]; ok {
			if previousRaw, previousOK := previousRaws[packageID]; !previousOK || !nullStringEqual(currentRaw, previousRaw) {
				statements = append(statements, rawUpsertStatement(packageID, currentRaw))
			}
		} else if _, previousOK := previousRaws[packageID]; previousOK {
			statements = append(statements, fmt.Sprintf("DELETE FROM catalog_packages_raw WHERE package_id = %s;", sqlText(packageID)))
		}

		currentPackageInstallers := currentInstallers[packageID]
		previousPackageInstallers := previousInstallers[packageID]

		currentInstallerIDs := make([]int64, 0, len(currentPackageInstallers))
		for id := range currentPackageInstallers {
			currentInstallerIDs = append(currentInstallerIDs, id)
		}
		sort.Slice(currentInstallerIDs, func(i, j int) bool { return currentInstallerIDs[i] < currentInstallerIDs[j] })

		for _, installerID := range currentInstallerIDs {
			currentInstaller := currentPackageInstallers[installerID]
			previousInstaller, previousOK := previousPackageInstallers[installerID]
			if !previousOK {
				statements = append(statements, installerUpsertStatement(currentInstaller))
				continue
			}
			if !installerRecordsEqual(currentInstaller, previousInstaller) {
				statements = append(statements, installerUpdateStatement(currentInstaller))
			}
		}

		removedInstallerIDs := make([]int64, 0)
		for installerID := range previousPackageInstallers {
			if _, ok := currentPackageInstallers[installerID]; !ok {
				removedInstallerIDs = append(removedInstallerIDs, installerID)
			}
		}
		sort.Slice(removedInstallerIDs, func(i, j int) bool { return removedInstallerIDs[i] < removedInstallerIDs[j] })
		for _, installerID := range removedInstallerIDs {
			statements = append(statements, fmt.Sprintf("DELETE FROM catalog_installers WHERE id = %d;", installerID))
		}
	}

	for _, packageID := range removedPackages {
		statements = append(statements, fmt.Sprintf("DELETE FROM catalog_packages WHERE id = %s;", sqlText(packageID)))
	}

	statements = append(statements, "COMMIT;")

	return strings.Join(statements, "\n") + "\n", nil
}

func loadCatalogSnapshot(db *sql.DB) (map[string]packageRecord, map[string]sql.NullString, map[string]map[int64]installerRecord, error) {
	packages, err := loadPackageRows(db)
	if err != nil {
		return nil, nil, nil, err
	}

	rawRows, err := loadRawRows(db)
	if err != nil {
		return nil, nil, nil, err
	}

	installerRows, err := loadInstallerRows(db)
	if err != nil {
		return nil, nil, nil, err
	}

	return packages, rawRows, installerRows, nil
}

func loadPackageRows(db *sql.DB) (map[string]packageRecord, error) {
	rows, err := db.Query(`
SELECT rowid, id, name, version, source, namespace, source_id, description, homepage, license, publisher, locale, moniker, tags, bin, created_at, updated_at
FROM catalog_packages
ORDER BY id ASC`)
	if err != nil {
		return nil, fmt.Errorf("failed to query catalog packages: %w", err)
	}
	defer rows.Close()

	result := make(map[string]packageRecord)
	for rows.Next() {
		var record packageRecord
		if err := rows.Scan(
			&record.RowID,
			&record.ID,
			&record.Name,
			&record.Version,
			&record.Source,
			&record.Namespace,
			&record.SourceID,
			&record.Description,
			&record.Homepage,
			&record.License,
			&record.Publisher,
			&record.Locale,
			&record.Moniker,
			&record.Tags,
			&record.Bin,
			&record.CreatedAt,
			&record.UpdatedAt,
		); err != nil {
			return nil, fmt.Errorf("failed to scan catalog package row: %w", err)
		}
		result[record.ID] = record
	}

	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("failed to iterate catalog package rows: %w", err)
	}

	return result, nil
}

func loadRawRows(db *sql.DB) (map[string]sql.NullString, error) {
	rows, err := db.Query(`
SELECT package_id, raw
FROM catalog_packages_raw
ORDER BY package_id ASC`)
	if err != nil {
		return nil, fmt.Errorf("failed to query catalog package raw rows: %w", err)
	}
	defer rows.Close()

	result := make(map[string]sql.NullString)
	for rows.Next() {
		var packageID string
		var raw sql.NullString
		if err := rows.Scan(&packageID, &raw); err != nil {
			return nil, fmt.Errorf("failed to scan catalog package raw row: %w", err)
		}
		result[packageID] = raw
	}

	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("failed to iterate catalog package raw rows: %w", err)
	}

	return result, nil
}

func loadInstallerRows(db *sql.DB) (map[string]map[int64]installerRecord, error) {
	rows, err := db.Query(`
SELECT id, package_id, url, hash, hash_algorithm, installer_type, installer_switches, scope, arch, kind, nested_kind
FROM catalog_installers
ORDER BY package_id ASC, id ASC`)
	if err != nil {
		return nil, fmt.Errorf("failed to query catalog installers: %w", err)
	}
	defer rows.Close()

	result := make(map[string]map[int64]installerRecord)
	for rows.Next() {
		var record installerRecord
		if err := rows.Scan(
			&record.ID,
			&record.PackageID,
			&record.URL,
			&record.Hash,
			&record.HashAlgorithm,
			&record.InstallerType,
			&record.InstallerSwitches,
			&record.Scope,
			&record.Arch,
			&record.Kind,
			&record.NestedKind,
		); err != nil {
			return nil, fmt.Errorf("failed to scan catalog installer row: %w", err)
		}

		if _, ok := result[record.PackageID]; !ok {
			result[record.PackageID] = make(map[int64]installerRecord)
		}
		result[record.PackageID][record.ID] = record
	}

	if err := rows.Err(); err != nil {
		return nil, fmt.Errorf("failed to iterate catalog installer rows: %w", err)
	}

	return result, nil
}

func packageRecordsEqual(left, right packageRecord) bool {
	return left.RowID == right.RowID &&
		left.ID == right.ID &&
		left.Name == right.Name &&
		left.Version == right.Version &&
		left.Source == right.Source &&
		nullStringEqual(left.Namespace, right.Namespace) &&
		left.SourceID == right.SourceID &&
		nullStringEqual(left.Description, right.Description) &&
		nullStringEqual(left.Homepage, right.Homepage) &&
		nullStringEqual(left.License, right.License) &&
		nullStringEqual(left.Publisher, right.Publisher) &&
		nullStringEqual(left.Locale, right.Locale) &&
		nullStringEqual(left.Moniker, right.Moniker) &&
		nullStringEqual(left.Tags, right.Tags) &&
		nullStringEqual(left.Bin, right.Bin) &&
		left.CreatedAt == right.CreatedAt &&
		left.UpdatedAt == right.UpdatedAt
}

func installerRecordsEqual(left, right installerRecord) bool {
	return left.ID == right.ID &&
		left.PackageID == right.PackageID &&
		left.URL == right.URL &&
		nullStringEqual(left.Hash, right.Hash) &&
		left.HashAlgorithm == right.HashAlgorithm &&
		left.InstallerType == right.InstallerType &&
		nullStringEqual(left.InstallerSwitches, right.InstallerSwitches) &&
		nullStringEqual(left.Scope, right.Scope) &&
		left.Arch == right.Arch &&
		left.Kind == right.Kind &&
		nullStringEqual(left.NestedKind, right.NestedKind)
}

func nullStringEqual(left, right sql.NullString) bool {
	return left.Valid == right.Valid && left.String == right.String
}

func packageUpsertStatement(record packageRecord) string {
	return fmt.Sprintf(
		"INSERT OR REPLACE INTO catalog_packages (rowid, id, name, version, source, namespace, source_id, description, homepage, license, publisher, locale, moniker, tags, bin, created_at, updated_at) VALUES (%d, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s);",
		record.RowID,
		sqlText(record.ID),
		sqlText(record.Name),
		sqlText(record.Version),
		sqlText(record.Source),
		sqlNullableText(record.Namespace.String),
		sqlText(record.SourceID),
		sqlNullableText(record.Description.String),
		sqlNullableText(record.Homepage.String),
		sqlNullableText(record.License.String),
		sqlNullableText(record.Publisher.String),
		sqlNullableText(record.Locale.String),
		sqlNullableText(record.Moniker.String),
		sqlNullableText(record.Tags.String),
		sqlNullableText(record.Bin.String),
		sqlText(record.CreatedAt),
		sqlText(record.UpdatedAt),
	)
}

func rawUpsertStatement(packageID string, raw sql.NullString) string {
	return fmt.Sprintf(
		"INSERT OR REPLACE INTO catalog_packages_raw (package_id, raw) VALUES (%s, %s);",
		sqlText(packageID),
		sqlNullableText(raw.String),
	)
}

func installerUpsertStatement(record installerRecord) string {
	return fmt.Sprintf(
		"INSERT OR REPLACE INTO catalog_installers (id, package_id, url, hash, hash_algorithm, installer_type, installer_switches, scope, arch, kind, nested_kind) VALUES (%d, %s, %s, %s, %s, %s, %s, %s, %s, %s, %s);",
		record.ID,
		sqlText(record.PackageID),
		sqlText(record.URL),
		sqlNullableText(record.Hash.String),
		sqlText(record.HashAlgorithm),
		sqlText(record.InstallerType),
		sqlNullableText(record.InstallerSwitches.String),
		sqlNullableText(record.Scope.String),
		sqlText(record.Arch),
		sqlText(record.Kind),
		sqlNullableText(record.NestedKind.String),
	)
}

func installerUpdateStatement(record installerRecord) string {
	return fmt.Sprintf(
		"UPDATE catalog_installers SET package_id = %s, url = %s, hash = %s, hash_algorithm = %s, installer_type = %s, installer_switches = %s, scope = %s, arch = %s, kind = %s, nested_kind = %s WHERE id = %d;",
		sqlText(record.PackageID),
		sqlText(record.URL),
		sqlNullableText(record.Hash.String),
		sqlText(record.HashAlgorithm),
		sqlText(record.InstallerType),
		sqlNullableText(record.InstallerSwitches.String),
		sqlNullableText(record.Scope.String),
		sqlText(record.Arch),
		sqlText(record.Kind),
		sqlNullableText(record.NestedKind.String),
		record.ID,
	)
}

func openSQLiteReadOnly(path string) (*sql.DB, error) {
	dsn, err := sqliteDSN(path)
	if err != nil {
		return nil, err
	}

	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, fmt.Errorf("failed to open sqlite database: %w", err)
	}

	return db, nil
}

func sqliteDSN(dbPath string) (string, error) {
	absPath, err := filepath.Abs(dbPath)
	if err != nil {
		return "", fmt.Errorf("failed to resolve sqlite database path: %w", err)
	}
	uriPath := filepath.ToSlash(absPath)
	if len(uriPath) >= 2 && uriPath[1] == ':' {
		uriPath = "/" + uriPath
	}

	return (&url.URL{
		Scheme:   "file",
		Path:     uriPath,
		RawQuery: "mode=ro",
	}).String(), nil
}
