package main

import (
	"context"
	"fmt"
	"log/slog"
	"os"
	"os/signal"
	"path/filepath"
	"strings"
	"syscall"

	"github.com/klauspost/compress/zstd"
	"winbrew/infra/internal/config"
	"winbrew/infra/internal/retry"
	"winbrew/infra/pkg/cdn"
	"winbrew/infra/pkg/cdn/r2"
	"winbrew/infra/pkg/db"
)

const (
	cacheDirName  = "winbrew-cache"
	dbFileName    = "packages.db"
	compressedExt = ".zst"
)

func main() {
	if err := run(); err != nil {
		slog.Error("publisher failed", "err", err)
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

	provider, err := buildProvider(cfg)
	if err != nil {
		return fmt.Errorf("failed to build CDN provider: %w", err)
	}

	cacheDir := filepath.Join(os.TempDir(), cacheDirName)
	if err := os.MkdirAll(cacheDir, 0o755); err != nil {
		return fmt.Errorf("failed to create cache dir: %w", err)
	}

	dbPath := filepath.Join(cacheDir, dbFileName)
	if _, err := os.Stat(dbPath); err != nil {
		return fmt.Errorf("database not found at %s: %w", dbPath, err)
	}

	compressedPath := dbPath + compressedExt
	defer func() {
		_ = os.Remove(compressedPath)
	}()

	slog.Info("compressing database", "db", dbPath, "output", compressedPath)
	if err := db.CompressFile(dbPath, compressedPath, zstd.SpeedBestCompression); err != nil {
		return fmt.Errorf("failed to compress database: %w", err)
	}

	key := filepath.Base(compressedPath)
	uploadCtx, cancelUpload := context.WithTimeout(ctx, cfg.Timeout.Upload)
	defer cancelUpload()

	slog.Info("uploading artifact", "key", key)
	if err := retry.Do(uploadCtx, cfg.Retry.Max, cfg.Retry.Backoff, func() error {
		return provider.Upload(uploadCtx, key, compressedPath)
	}); err != nil {
		return fmt.Errorf("failed to upload artifact: %w", err)
	}

	slog.Info("publish complete", "url", provider.PublicURL(key))
	return nil
}

func buildProvider(cfg *config.Config) (cdn.Provider, error) {
	switch strings.ToLower(cfg.CDN.Provider) {
	case "r2":
		accessKeyID, secretAccessKey, err := cfg.CDN.ResolveCredentials()
		if err != nil {
			return nil, fmt.Errorf("failed to resolve r2 credentials: %w", err)
		}

		return r2.New(cfg.CDN.Endpoint, cfg.CDN.Bucket, accessKeyID, secretAccessKey)
	default:
		return nil, fmt.Errorf("unsupported CDN provider %q", cfg.CDN.Provider)
	}
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
