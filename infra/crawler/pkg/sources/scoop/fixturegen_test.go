package scoop

import (
	"context"
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"runtime"
	"sort"
	"testing"

	"infra/crawler/pkg/normalize"
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
	selected, err := collectRealScoopPackages(context.Background(), cacheDir)
	if err != nil {
		t.Fatalf("collectRealScoopPackages() error = %v", err)
	}

	if err := writeScoopFixture(filepath.Join(fixturesDir, "scoop_packages.jsonl"), selected); err != nil {
		t.Fatalf("writeScoopFixture() error = %v", err)
	}
}

func collectRealScoopPackages(ctx context.Context, cacheDir string) ([]normalize.Package, error) {
	buckets := []struct {
		name string
		url  string
	}{
		{name: "main", url: "https://github.com/ScoopInstaller/Main"},
		{name: "extras", url: "https://github.com/ScoopInstaller/Extras"},
	}

	preferrredManifests := []string{"vscode.json", "neovim.json", "git.json", "7zip.json"}
	selected := make([]normalize.Package, 0, 2)

	for _, bucket := range buckets {
		repoDir := filepath.Join(cacheDir, bucket.name)
		if err := syncRepo(ctx, bucket.url, repoDir); err != nil {
			return nil, fmt.Errorf("sync %s bucket: %w", bucket.name, err)
		}

		manifestDir := filepath.Join(repoDir, "bucket")
		for _, manifestName := range preferrredManifests {
			if len(selected) >= 2 {
				break
			}

			if _, err := os.Stat(filepath.Join(manifestDir, manifestName)); err != nil {
				continue
			}

			pkg, err := readManifest(ctx, bucket.name, manifestDir, manifestName)
			if err != nil {
				return nil, fmt.Errorf("read %s/%s: %w", bucket.name, manifestName, err)
			}
			selected = append(selected, pkg)
		}

		if len(selected) >= 2 {
			break
		}
	}

	if len(selected) < 2 {
		return nil, fmt.Errorf("expected at least 2 real scoop packages, got %d", len(selected))
	}

	sort.SliceStable(selected, func(i, j int) bool {
		return selected[i].ID < selected[j].ID
	})

	return selected, nil
}

func writeScoopFixture(path string, packages []normalize.Package) error {
	file, err := os.Create(path)
	if err != nil {
		return err
	}
	defer file.Close()

	encoder := json.NewEncoder(file)
	for _, pkg := range packages {
		if err := encoder.Encode(scoopEnvelopeFromPackage(pkg)); err != nil {
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
