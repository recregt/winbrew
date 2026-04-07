package config

import (
	"context"
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

func TestParseExpandsEnvironmentVariables(t *testing.T) {
	t.Setenv("WINBREW_LOG_LEVEL", "debug")
	t.Setenv("WINBREW_SOURCE", "winget")

	cfg, err := Parse(strings.NewReader(`
sources:
  - ${WINBREW_SOURCE}
logLevel: ${WINBREW_LOG_LEVEL}
`))
	if err != nil {
		t.Fatalf("Parse() error = %v, want nil", err)
	}

	if got, want := cfg.LogLevel, "debug"; got != want {
		t.Fatalf("LogLevel = %q, want %q", got, want)
	}
	if got, want := cfg.Sources, []string{"winget"}; len(got) != len(want) || got[0] != want[0] {
		t.Fatalf("Sources = %#v, want %#v", got, want)
	}
}

func TestParseReportsMultipleValidationErrors(t *testing.T) {
	t.Parallel()

	_, err := Parse(strings.NewReader(`
sources:
  - ""
  - winget
  - winget
  - invalid
logLevel: trace
timeout:
  fetch: -1s
retry:
  max: -2
  backoff: -1s
`))
	if err == nil {
		t.Fatal("Parse() error = nil, want non-nil")
	}

	wantFragments := []string{
		"config validation failed with",
		"sources[0]: empty source name",
		"duplicate source: winget",
		"unknown source: invalid",
		"invalid log level",
		"timeout.fetch cannot be negative",
		"retry.max cannot be negative",
		"retry.backoff must be positive",
	}
	for _, fragment := range wantFragments {
		if !strings.Contains(err.Error(), fragment) {
			t.Fatalf("Parse() error = %q, want fragment %q", err.Error(), fragment)
		}
	}
}

func TestParseRejectsZeroRetryBackoff(t *testing.T) {
	t.Parallel()

	_, err := Parse(strings.NewReader(`
sources:
  - winget
retry:
  backoff: 0s
`))
	if err == nil {
		t.Fatal("Parse() error = nil, want non-nil")
	}
	if !strings.Contains(err.Error(), "retry.backoff must be positive") {
		t.Fatalf("Parse() error = %q, want backoff validation", err.Error())
	}
}

func TestLoadContextHonorsCancellation(t *testing.T) {
	t.Parallel()

	dir := t.TempDir()
	path := filepath.Join(dir, "config.yaml")
	if err := os.WriteFile(path, []byte("sources:\n  - winget\n"), 0o644); err != nil {
		t.Fatalf("WriteFile() error = %v", err)
	}

	ctx, cancel := context.WithCancel(context.Background())
	cancel()

	_, err := LoadContext(ctx, path)
	if err == nil {
		t.Fatal("LoadContext() error = nil, want non-nil")
	}
	if !strings.Contains(err.Error(), "context canceled") {
		t.Fatalf("LoadContext() error = %q, want context cancellation", err.Error())
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
