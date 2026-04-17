package winget

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"sync"
	"testing"
	"time"
)

func TestRefreshParserFixtures(t *testing.T) {
	if os.Getenv("WINBREW_REFRESH_PARSER_FIXTURES") == "" {
		t.Skip("set WINBREW_REFRESH_PARSER_FIXTURES=1 to regenerate parser fixtures")
	}

	fixturesDir := parserFixturesDir(t)
	if err := os.MkdirAll(fixturesDir, 0o750); err != nil {
		t.Fatalf("MkdirAll() error = %v", err)
	}

	cacheDir := t.TempDir()
	packages, err := collectRealWingetPackages(context.Background(), cacheDir)
	if err != nil {
		t.Fatalf("collectRealWingetPackages() error = %v", err)
	}

	if err := writeWingetFixture(filepath.Join(fixturesDir, "winget_packages.jsonl"), packages); err != nil {
		t.Fatalf("writeWingetFixture() error = %v", err)
	}
}

func collectRealWingetPackages(ctx context.Context, cacheDir string) ([]wingetEnvelope, error) {
	client := &http.Client{Timeout: 10 * time.Minute}
	source, err := New(client, filepath.Join(cacheDir, "winget"))
	if err != nil {
		return nil, fmt.Errorf("create source: %w", err)
	}
	defer source.Close()

	dbPath := filepath.Join(cacheDir, "winget_source.db")
	if err := source.DownloadSourceDB(ctx, dbPath); err != nil {
		return nil, fmt.Errorf("download source db: %w", err)
	}

	rows, err := readWingetIndexRows(ctx, dbPath)
	if err != nil {
		return nil, fmt.Errorf("read index rows: %w", err)
	}

	const targetCount = 371
	const workerCount = 8

	wantedIDs := []string{
		"Microsoft.VisualStudioCode",
		"Microsoft.PowerToys",
		"Git.Git",
		"Microsoft.WindowsTerminal",
	}

	candidateRows := make([]wingetIndexRow, 0, len(rows))
	selectedIDs := make(map[string]struct{}, len(rows))
	addCandidate := func(row wingetIndexRow) {
		if row.id == "" {
			return
		}
		if _, seen := selectedIDs[row.id]; seen {
			return
		}
		selectedIDs[row.id] = struct{}{}
		candidateRows = append(candidateRows, row)
	}

	for _, wantedID := range wantedIDs {
		row, ok := findWingetRow(rows, wantedID)
		if !ok {
			continue
		}
		addCandidate(row)
	}

	for _, row := range rows {
		addCandidate(row)
	}

	if len(candidateRows) < targetCount {
		return nil, fmt.Errorf("expected at least %d real winget packages, got %d", targetCount, len(candidateRows))
	}

	ctx, cancel := context.WithCancel(ctx)
	defer cancel()

	rowCh := make(chan wingetIndexRow)
	resultCh := make(chan wingetEnvelope, workerCount)
	var workers sync.WaitGroup

	for i := 0; i < workerCount; i++ {
		workers.Add(1)
		go func() {
			defer workers.Done()
			for row := range rowCh {
				pkg, err := source.buildPackageSnapshot(ctx, row, 3, 2*time.Second)
				if err != nil {
					continue
				}

				envelope := wingetEnvelope{
					SchemaVersion: wingetEnvelopeSchemaVersion,
					Source:        sourceName,
					Kind:          wingetEnvelopeKind,
					Payload:       pkg,
				}

				select {
				case resultCh <- envelope:
				case <-ctx.Done():
					return
				}
			}
		}()
	}

	go func() {
		workers.Wait()
		close(resultCh)
	}()

	go func() {
		defer close(rowCh)
		for _, row := range candidateRows {
			select {
			case <-ctx.Done():
				return
			case rowCh <- row:
			}
		}
	}()

	selected := make([]wingetEnvelope, 0, targetCount)
	for envelope := range resultCh {
		if len(selected) >= targetCount {
			continue
		}

		selected = append(selected, envelope)
		if len(selected) >= targetCount {
			cancel()
		}
	}

	if len(selected) < targetCount {
		return nil, fmt.Errorf("expected at least %d real winget packages, got %d", targetCount, len(selected))
	}

	sort.SliceStable(selected, func(i, j int) bool {
		return selected[i].Payload.ID < selected[j].Payload.ID
	})

	return selected, nil
}

func findWingetRow(rows []wingetIndexRow, wantedID string) (wingetIndexRow, bool) {
	for _, row := range rows {
		if row.id == wantedID {
			return row, true
		}
	}

	return wingetIndexRow{}, false
}

func writeWingetFixture(path string, packages []wingetEnvelope) error {
	file, err := os.Create(path)
	if err != nil {
		return err
	}
	defer file.Close()

	encoder := json.NewEncoder(file)
	for _, pkg := range packages {
		if err := encoder.Encode(pkg); err != nil {
			return err
		}
	}

	return nil
}

func parserFixturesDir(t *testing.T) string {
	t.Helper()

	_, file, _, ok := runtime.Caller(0)
	if !ok {
		t.Fatal("runtime.Caller() failed")
	}

	repoRoot := filepath.Clean(filepath.Join(filepath.Dir(file), "..", "..", "..", "..", ".."))
	return filepath.Join(repoRoot, "infra", "parser", "tests", "fixtures")
}
