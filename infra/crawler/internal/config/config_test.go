package config

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"
)

func TestParseAppliesDefaultsAndNormalization(t *testing.T) {
	t.Parallel()

	cfg, err := Parse(strings.NewReader(`
sources:
  - Winget
  - scoop
logLevel: WARN
timeout:
  fetch: 30s
retry:
  max: 0
`))
	if err != nil {
		t.Fatalf("Parse() error = %v, want nil", err)
	}

	if got, want := cfg.LogLevel, "warn"; got != want {
		t.Fatalf("LogLevel = %q, want %q", got, want)
	}
	if got, want := cfg.Sources, []string{"winget", "scoop"}; len(got) != len(want) || got[0] != want[0] || got[1] != want[1] {
		t.Fatalf("Sources = %#v, want %#v", got, want)
	}
	if got, want := cfg.Timeout.Fetch, 30*time.Second; got != want {
		t.Fatalf("Timeout.Fetch = %v, want %v", got, want)
	}
	if got, want := cfg.Retry.Max, 0; got != want {
		t.Fatalf("Retry.Max = %d, want %d", got, want)
	}
	if got, want := cfg.Retry.Backoff, defaultRetryBackoff; got != want {
		t.Fatalf("Retry.Backoff = %v, want %v", got, want)
	}
}

func TestParseRejectsUnknownFields(t *testing.T) {
	t.Parallel()

	_, err := Parse(strings.NewReader(`
sources:
  - winget
log_level: debug
`))
	if err == nil {
		t.Fatal("Parse() error = nil, want non-nil")
	}
	if !strings.Contains(err.Error(), "log_level") {
		t.Fatalf("Parse() error = %q, want mention of unknown field", err.Error())
	}
}

func TestParseRejectsEmptyInput(t *testing.T) {
	t.Parallel()

	_, err := Parse(strings.NewReader(""))
	if err == nil {
		t.Fatal("Parse() error = nil, want non-nil")
	}
	if !strings.Contains(err.Error(), "failed to parse config") {
		t.Fatalf("Parse() error = %q, want parse failure", err.Error())
	}
}

func TestLoadReadsConfigFile(t *testing.T) {
	t.Parallel()

	dir := t.TempDir()
	path := filepath.Join(dir, "config.yaml")
	if err := os.WriteFile(path, []byte("sources:\n  - winget\n"), 0o644); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}

	cfg, err := Load(path)
	if err != nil {
		t.Fatalf("Load() error = %v, want nil", err)
	}
	if got, want := cfg.Sources, []string{"winget"}; len(got) != len(want) || got[0] != want[0] {
		t.Fatalf("Sources = %#v, want %#v", got, want)
	}
}
