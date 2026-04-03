package main

import (
	"context"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"strings"
	"syscall"

	"winbrew/infra/internal/config"
	"winbrew/infra/internal/retry"
	"winbrew/infra/pkg/db"
	"winbrew/infra/pkg/normalize"
	"winbrew/infra/pkg/sources"
	"winbrew/infra/pkg/sources/scoop"
	"winbrew/infra/pkg/sources/winget"
)

func main() {
	if err := run(); err != nil {
		slog.Error("crawler failed", "err", err)
		os.Exit(1)
	}
}

func run() error {
	cfg, err := config.Load("config.yaml")
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	level := parseLogLevel(cfg.LogLevel)
	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: level})))

	ctx, cancel := signal.NotifyContext(context.Background(), syscall.SIGINT, syscall.SIGTERM)
	defer cancel()

	httpClient := &http.Client{Timeout: cfg.Timeout.Fetch}
	cacheDir := filepath.Join(os.TempDir(), "winbrew-cache")

	srcs, err := buildSources(cfg, httpClient, cacheDir)
	if err != nil {
		return fmt.Errorf("failed to build sources: %w", err)
	}

	dbPath, err := defaultCatalogDBPath()
	if err != nil {
		return fmt.Errorf("failed to resolve catalog db path: %w", err)
	}

	if err := os.MkdirAll(filepath.Dir(dbPath), 0o755); err != nil {
		return fmt.Errorf("failed to create db directory: %w", err)
	}

	writer, err := db.Open(dbPath)
	if err != nil {
		return fmt.Errorf("failed to open db: %w", err)
	}
	defer writer.Close()

	for _, src := range srcs {
		slog.Info("fetching source", "name", src.Name())

		var pkgs []normalize.Package
		err := retry.Do(ctx, cfg.Retry.Max, cfg.Retry.Backoff, func() error {
			var err error
			pkgs, err = src.Fetch(ctx)
			return err
		})
		if err != nil {
			return fmt.Errorf("source %s: %w", src.Name(), err)
		}

		slog.Info("writing packages", "source", src.Name(), "count", len(pkgs))

		if err := retry.Do(ctx, cfg.Retry.Max, cfg.Retry.Backoff, func() error {
			return writer.WritePackages(ctx, pkgs)
		}); err != nil {
			return fmt.Errorf("failed to write packages from %s: %w", src.Name(), err)
		}
	}
	slog.Info("crawl complete", "db", dbPath)
	return nil
}

func defaultCatalogDBPath() (string, error) {
	localAppData := os.Getenv("LOCALAPPDATA")
	if localAppData == "" {
		return "", fmt.Errorf("LOCALAPPDATA environment variable is not set")
	}

	return filepath.Join(localAppData, "winbrew", "data", "db", "catalog.db"), nil
}

func buildSources(cfg *config.Config, httpClient *http.Client, cacheDir string) ([]sources.Source, error) {
	var srcs []sources.Source

	for _, name := range cfg.Sources {
		switch strings.ToLower(name) {
		case "winget":
			s, err := winget.New(httpClient, filepath.Join(cacheDir, "winget"))
			if err != nil {
				return nil, fmt.Errorf("winget: %w", err)
			}
			srcs = append(srcs, s)
		case "scoop":
			s, err := scoop.New(filepath.Join(cacheDir, "scoop"))
			if err != nil {
				return nil, fmt.Errorf("scoop: %w", err)
			}
			srcs = append(srcs, s)
		default:
			return nil, fmt.Errorf("unknown source: %s", name)
		}
	}

	return srcs, nil
}

func parseLogLevel(level string) slog.Level {
	switch strings.ToLower(level) {
	case "debug":
		return slog.LevelDebug
	case "warn":
		return slog.LevelWarn
	case "error":
		return slog.LevelError
	default:
		return slog.LevelInfo
	}
}
