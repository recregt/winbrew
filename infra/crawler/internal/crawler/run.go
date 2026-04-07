package crawler

import (
	"context"
	"fmt"
	"log/slog"
	"net/http"
	"os"
	"path/filepath"
	"strings"

	"golang.org/x/sync/errgroup"

	"winbrew/infra/internal/config"
	"winbrew/infra/internal/retry"
	"winbrew/infra/pkg/sources/scoop"
	"winbrew/infra/pkg/sources/winget"
)

type crawlerSources struct {
	scoop  *scoop.Source
	winget *winget.Source
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

func runPipeline(ctx context.Context, cfg *config.Config, srcs crawlerSources, wingetOutPath string) error {
	if err := os.MkdirAll(filepath.Dir(wingetOutPath), 0o755); err != nil {
		return fmt.Errorf("failed to create winget staging dir: %w", err)
	}

	group, groupCtx := errgroup.WithContext(ctx)

	group.Go(func() error {
		if srcs.scoop == nil {
			return fmt.Errorf("scoop source is not configured")
		}

		slog.Info("streaming source", "name", srcs.scoop.Name())
		if err := srcs.scoop.WriteJSONL(groupCtx, os.Stdout, cfg.Retry.Max, cfg.Retry.Backoff); err != nil {
			if groupCtx.Err() != nil {
				return fmt.Errorf("source %s cancelled: %w", srcs.scoop.Name(), groupCtx.Err())
			}
			return fmt.Errorf("failed to stream packages from %s: %w", srcs.scoop.Name(), err)
		}

		return nil
	})

	group.Go(func() error {
		if srcs.winget == nil {
			return fmt.Errorf("winget source is not configured")
		}

		slog.Info("downloading staged source db", "name", srcs.winget.Name(), "dst", wingetOutPath)
		if err := retry.Do(groupCtx, cfg.Retry.Max, cfg.Retry.Backoff, func() error {
			return srcs.winget.DownloadSourceDB(groupCtx, wingetOutPath)
		}); err != nil {
			if groupCtx.Err() != nil {
				return fmt.Errorf("source %s cancelled: %w", srcs.winget.Name(), groupCtx.Err())
			}
			return fmt.Errorf("failed to stage %s source db: %w", srcs.winget.Name(), err)
		}

		return nil
	})

	if err := group.Wait(); err != nil {
		return err
	}

	slog.Info("pipeline complete", "winget_db", wingetOutPath)
	return nil
}

func buildSources(cfg *config.Config, httpClient *http.Client, cacheDir string) (crawlerSources, error) {
	var srcs crawlerSources

	for _, name := range cfg.Sources {
		switch name {
		case "winget":
			s, err := winget.New(httpClient, filepath.Join(cacheDir, "winget"))
			if err != nil {
				return crawlerSources{}, fmt.Errorf("winget: %w", err)
			}
			srcs.winget = s
		case "scoop":
			s, err := scoop.New(filepath.Join(cacheDir, "scoop"))
			if err != nil {
				return crawlerSources{}, fmt.Errorf("scoop: %w", err)
			}
			srcs.scoop = s
		default:
			return crawlerSources{}, fmt.Errorf("unknown source: %s", name)
		}
	}

	if srcs.scoop == nil || srcs.winget == nil {
		return crawlerSources{}, fmt.Errorf("both scoop and winget sources must be configured")
	}

	return srcs, nil
}
