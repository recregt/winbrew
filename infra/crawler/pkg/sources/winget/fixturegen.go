package winget

import (
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"sync"
	"time"
)

const fixtureWorkerCount = 8

var fixtureWantedIDs = []string{
	"Microsoft.VisualStudioCode",
	"Microsoft.PowerToys",
	"Git.Git",
	"Microsoft.WindowsTerminal",
}

func GenerateFixtures(ctx context.Context, outputPath string, count int) error {
	outputPath = strings.TrimSpace(outputPath)
	if outputPath == "" {
		return fmt.Errorf("output path cannot be empty")
	}
	if count <= 0 {
		return fmt.Errorf("count must be greater than zero")
	}

	cacheDir, err := os.MkdirTemp("", "winbrew-winget-fixtures-*")
	if err != nil {
		return fmt.Errorf("create temp cache dir: %w", err)
	}
	defer func() {
		_ = os.RemoveAll(cacheDir)
	}()

	client := &http.Client{Timeout: 10 * time.Minute}
	source, err := New(client, filepath.Join(cacheDir, "winget"))
	if err != nil {
		return fmt.Errorf("create source: %w", err)
	}
	defer source.Close()

	dbPath := filepath.Join(cacheDir, "winget_source.db")
	if err := source.DownloadSourceDB(ctx, dbPath); err != nil {
		return fmt.Errorf("download source db: %w", err)
	}

	packages, err := collectRealWingetPackages(ctx, source, dbPath, count)
	if err != nil {
		return err
	}

	if err := writeWingetFixture(outputPath, packages); err != nil {
		return fmt.Errorf("write fixture: %w", err)
	}

	return nil
}

func collectRealWingetPackages(ctx context.Context, source *Source, dbPath string, targetCount int) ([]wingetEnvelope, error) {
	rows, err := readWingetIndexRows(ctx, dbPath)
	if err != nil {
		return nil, fmt.Errorf("read index rows: %w", err)
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

	for _, wantedID := range fixtureWantedIDs {
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
	resultCh := make(chan wingetEnvelope, fixtureWorkerCount)
	var workers sync.WaitGroup

	for i := 0; i < fixtureWorkerCount; i++ {
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
	if err := os.MkdirAll(filepath.Dir(path), 0o750); err != nil {
		return fmt.Errorf("create fixture dir: %w", err)
	}

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
