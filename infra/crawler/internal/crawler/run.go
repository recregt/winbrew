package crawler

import (
	"context"
	"fmt"
	"io"
	"log/slog"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"winbrew/infra/internal/config"
	"winbrew/infra/internal/retry"
	"winbrew/infra/pkg/sources"
	"winbrew/infra/pkg/sources/scoop"
	"winbrew/infra/pkg/sources/winget"
)

type wingetStager interface {
	DownloadSourceDB(ctx context.Context, dst string) error
}

type scoopStreamer interface {
	WriteJSONL(ctx context.Context, w io.Writer, maxAttempts int, backoff time.Duration) error
}

func Run(ctx context.Context, configPath, wingetOutPath string) error {
	cfg, err := config.Load(configPath)
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	var level slog.Level
	if err := level.UnmarshalText([]byte(cfg.LogLevel)); err != nil {
		level = slog.LevelInfo
	}

	// Logs must stay off stdout because stdout is the pipeline data channel.
	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: level})))

	httpClient := &http.Client{Timeout: cfg.Timeout.Fetch}
	cacheBase, err := os.UserCacheDir()
	if err != nil {
		return fmt.Errorf("failed to resolve cache dir: %w", err)
	}
	cacheDir := filepath.Join(cacheBase, "winbrew")
	if err := os.MkdirAll(cacheDir, 0o755); err != nil {
		return fmt.Errorf("failed to create cache dir: %w", err)
	}

	srcs, err := buildSources(cfg, httpClient, cacheDir)
	if err != nil {
		return fmt.Errorf("failed to build sources: %w", err)
	}

	trimmed := strings.TrimSpace(wingetOutPath)
	if trimmed == "" {
		trimmed = filepath.Join("staging", "winget_source.db")
	}

	return runPipeline(ctx, cfg, srcs, trimmed)
}

func runPipeline(ctx context.Context, cfg *config.Config, srcs []sources.Source, wingetOutPath string) error {
	if err := os.MkdirAll(filepath.Dir(wingetOutPath), 0o755); err != nil {
		return fmt.Errorf("failed to create winget staging dir: %w", err)
	}

	doRetry := func(runCtx context.Context, fn func() error) error {
		return retry.Do(runCtx, cfg.Retry.Max, cfg.Retry.Backoff, fn)
	}

	for _, src := range srcs {
		switch src.Name() {
		case "scoop":
			streamer, ok := src.(scoopStreamer)
			if !ok {
				return fmt.Errorf("scoop source does not support streaming")
			}

			slog.Info("streaming source", "name", src.Name())
			if err := streamer.WriteJSONL(ctx, os.Stdout, cfg.Retry.Max, cfg.Retry.Backoff); err != nil {
				if ctx.Err() != nil {
					return fmt.Errorf("source %s cancelled: %w", src.Name(), ctx.Err())
				}
				return fmt.Errorf("failed to stream packages from %s: %w", src.Name(), err)
			}

		case "winget":
			downloader, ok := src.(wingetStager)
			if !ok {
				return fmt.Errorf("winget source does not support staged downloads")
			}

			slog.Info("downloading staged source db", "name", src.Name(), "dst", wingetOutPath)
			if err := doRetry(ctx, func() error {
				return downloader.DownloadSourceDB(ctx, wingetOutPath)
			}); err != nil {
				if ctx.Err() != nil {
					return fmt.Errorf("source %s cancelled: %w", src.Name(), ctx.Err())
				}
				return fmt.Errorf("failed to stage %s source db: %w", src.Name(), err)
			}

		default:
			return fmt.Errorf("unknown source: %s", src.Name())
		}
	}

	slog.Info("pipeline complete", "winget_db", wingetOutPath)
	return nil
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
