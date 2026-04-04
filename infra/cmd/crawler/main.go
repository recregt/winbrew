package main

import (
	"context"
	"flag"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"os/signal"
	"path/filepath"
	"strings"

	"winbrew/infra/internal/config"
	"winbrew/infra/internal/retry"
	"winbrew/infra/pkg/db"
	"winbrew/infra/pkg/normalize"
	"winbrew/infra/pkg/sources"
	"winbrew/infra/pkg/sources/scoop"
	"winbrew/infra/pkg/sources/winget"

	"golang.org/x/sync/errgroup"
)

func main() {
	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stdout, nil)))

	configPath := flag.String("config", "config.yaml", "path to configuration file")
	outputPath := flag.String("output", "", "path to the catalog database output file")
	flag.Parse()

	if err := run(*configPath, *outputPath); err != nil {
		slog.Error("crawler failed", "err", err)
		os.Exit(1)
	}
}

func run(configPath, outputPath string) error {
	cfg, err := config.Load(configPath)
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	var level slog.Level
	if err := level.UnmarshalText([]byte(cfg.LogLevel)); err != nil {
		level = slog.LevelInfo
	}

	// Upgraded to the configured level after config is loaded.
	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stdout, &slog.HandlerOptions{Level: level})))

	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt)
	defer cancel()

	httpClient := &http.Client{Timeout: cfg.Timeout.Fetch}
	cacheBase, err := os.UserCacheDir()
	if err != nil {
		return fmt.Errorf("failed to resolve cache dir: %w", err)
	}
	cacheDir := filepath.Join(cacheBase, "winbrew")
	if err := os.MkdirAll(cacheDir, 0o755); err != nil {
		return fmt.Errorf("failed to create cache dir: %w", err)
	}

	dbPath, err := resolveCatalogDBPath(outputPath)
	if err != nil {
		return err
	}

	srcs, err := buildSources(cfg, httpClient, cacheDir)
	if err != nil {
		return fmt.Errorf("failed to build sources: %w", err)
	}

	writer, err := db.Open(dbPath)
	if err != nil {
		return fmt.Errorf("failed to open db: %w", err)
	}
	defer writer.Close()

	doRetry := func(runCtx context.Context, fn func() error) error {
		return retry.Do(runCtx, cfg.Retry.Max, cfg.Retry.Backoff, fn)
	}

	g, gCtx := errgroup.WithContext(ctx)

	for _, src := range srcs {
		g.Go(func() error {
			slog.Info("fetching source", "name", src.Name())

			var pkgs []normalize.Package
			err := doRetry(gCtx, func() error {
				var err error
				pkgs, err = src.Fetch(gCtx)
				return err
			})
			if err != nil {
				if gCtx.Err() != nil {
					return fmt.Errorf("source %s cancelled: %w", src.Name(), gCtx.Err())
				}
				slog.Warn("skipping source after retries", "source", src.Name(), "err", err)
				return nil
			}

			slog.Info("writing packages", "source", src.Name(), "count", len(pkgs))

			if err := doRetry(gCtx, func() error {
				return writer.WritePackages(gCtx, pkgs)
			}); err != nil {
				return fmt.Errorf("failed to write packages from %s: %w", src.Name(), err)
			}

			return nil
		})
	}

	if err := g.Wait(); err != nil {
		return err
	}
	slog.Info("crawl complete", "db", dbPath)
	return nil
}

func resolveCatalogDBPath(outputPath string) (string, error) {
	if trimmed := strings.TrimSpace(outputPath); trimmed != "" {
		return filepath.Clean(trimmed), nil
	}

	if envPath, ok := os.LookupEnv("WINBREW_DB_PATH"); ok {
		if trimmed := strings.TrimSpace(envPath); trimmed != "" {
			return filepath.Clean(trimmed), nil
		}
	}

	cacheBase, err := os.UserCacheDir()
	if err != nil {
		return "", fmt.Errorf("failed to resolve cache dir: %w", err)
	}

	return filepath.Join(cacheBase, "winbrew", "db", "catalog.db"), nil
}

func buildSources(cfg *config.Config, httpClient *http.Client, cacheDir string) ([]sources.Source, error) {
	var srcs []sources.Source

	for _, name := range cfg.Sources {
		switch name {
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
