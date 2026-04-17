package crawler

import (
	"bufio"
	"context"
	"errors"
	"fmt"
	"log/slog"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"time"

	"golang.org/x/sync/errgroup"

	"infra/crawler/internal/config"
	"infra/crawler/internal/retry"
	"infra/crawler/pkg/sources/scoop"
	"infra/crawler/pkg/sources/winget"
)

type crawlerSources struct {
	scoop  *scoop.Source
	winget *winget.Source
}

func (s crawlerSources) Close() error {
	var errs []error

	if s.scoop != nil {
		if err := s.scoop.Close(); err != nil {
			errs = append(errs, fmt.Errorf("scoop: %w", err))
		}
	}
	if s.winget != nil {
		if err := s.winget.Close(); err != nil {
			errs = append(errs, fmt.Errorf("winget: %w", err))
		}
	}

	return errors.Join(errs...)
}

func Run(ctx context.Context, configPath, wingetOutPath string) error {
	pipelineStart := time.Now()
	cfg, err := config.LoadContext(ctx, configPath)
	if err != nil {
		return fmt.Errorf("failed to load config: %w", err)
	}

	var level slog.Level
	if err := level.UnmarshalText([]byte(cfg.LogLevel)); err != nil {
		level = slog.LevelInfo
	}

	// Logs must stay off stdout because stdout is the pipeline data channel.
	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, &slog.HandlerOptions{Level: level})))

	httpClient := &http.Client{
		Timeout:   cfg.Timeout.Fetch,
		Transport: tunedTransport(cfg.Timeout.Fetch),
	}

	cacheBase, err := os.UserCacheDir()
	if err != nil {
		return fmt.Errorf("failed to resolve cache dir: %w", err)
	}
	cacheDir := filepath.Join(cacheBase, "winbrew")
	if err := os.MkdirAll(cacheDir, 0o750); err != nil {
		return fmt.Errorf("failed to create cache dir: %w", err)
	}

	srcs, err := buildSources(cfg, httpClient, cacheDir)
	if err != nil {
		return fmt.Errorf("failed to build sources: %w", err)
	}

	defer func() {
		if err := srcs.Close(); err != nil {
			slog.Warn("failed to close sources", "err", err)
		}
	}()

	trimmed := strings.TrimSpace(wingetOutPath)
	if trimmed == "" {
		trimmed = filepath.Join("staging", "winget_source.jsonl")
	}

	slog.Info("crawler pipeline starting",
		"config", configPath,
		"sources", cfg.Sources,
		"log_level", cfg.LogLevel,
		"fetch_timeout", cfg.Timeout.Fetch,
		"retry_max", cfg.Retry.Max,
		"retry_backoff", cfg.Retry.Backoff,
		"cache_dir", cacheDir,
		"winget_out", trimmed,
	)

	if err := runPipeline(ctx, cfg, srcs, cacheDir, trimmed); err != nil {
		return err
	}

	slog.Info("crawler pipeline finished", "elapsed", time.Since(pipelineStart), "winget_out", trimmed)

	return nil
}

func tunedTransport(responseHeaderTimeout time.Duration) *http.Transport {
	transport := http.DefaultTransport.(*http.Transport).Clone()
	transport.Proxy = http.ProxyFromEnvironment
	transport.DialContext = (&net.Dialer{
		Timeout:   10 * time.Second,
		KeepAlive: 30 * time.Second,
	}).DialContext
	transport.ForceAttemptHTTP2 = true
	transport.MaxIdleConns = 100
	transport.MaxIdleConnsPerHost = 50
	transport.MaxConnsPerHost = 50
	transport.IdleConnTimeout = 90 * time.Second
	transport.TLSHandshakeTimeout = 10 * time.Second
	transport.ExpectContinueTimeout = time.Second
	transport.ResponseHeaderTimeout = cappedHeaderTimeout(responseHeaderTimeout)
	return transport
}

func cappedHeaderTimeout(fetchTimeout time.Duration) time.Duration {
	const maxHeaderTimeout = 10 * time.Second
	if fetchTimeout > 0 && fetchTimeout < maxHeaderTimeout {
		return fetchTimeout
	}

	return maxHeaderTimeout
}

func runPipeline(ctx context.Context, cfg *config.Config, srcs crawlerSources, cacheDir, wingetOutPath string) error {
	pipelineStart := time.Now()
	group, groupCtx := errgroup.WithContext(ctx)
	configuredSources := 0

	if srcs.scoop != nil {
		configuredSources++
		group.Go(func() (err error) {
			stageStart := time.Now()
			slog.Info("streaming source started", "name", srcs.scoop.Name())
			stdoutWriter := bufio.NewWriterSize(os.Stdout, 256*1024)
			defer func() {
				if flushErr := stdoutWriter.Flush(); flushErr != nil {
					if err == nil {
						err = fmt.Errorf("failed to flush stdout: %w", flushErr)
					} else {
						slog.Error("failed to flush stdout buffer", "err", flushErr)
					}
				}
			}()

			if err = srcs.scoop.WriteJSONL(groupCtx, stdoutWriter, cfg.Retry.Max, cfg.Retry.Backoff); err != nil {
				if groupCtx.Err() != nil {
					return fmt.Errorf("source %s cancelled: %w", srcs.scoop.Name(), groupCtx.Err())
				}
				slog.Error("streaming source failed", "name", srcs.scoop.Name(), "elapsed", time.Since(stageStart), "err", err)
				return fmt.Errorf("failed to stream packages from %s: %w", srcs.scoop.Name(), err)
			}

			slog.Info("streaming source finished", "name", srcs.scoop.Name(), "elapsed", time.Since(stageStart))

			return nil
		})
	}

	if srcs.winget != nil {
		configuredSources++
		group.Go(func() error {
			downloadStart := time.Now()
			sourceDBPath := filepath.Join(cacheDir, "winget", "winget_source.db")
			if err := os.MkdirAll(filepath.Dir(sourceDBPath), 0o750); err != nil {
				return fmt.Errorf("failed to create winget cache dir: %w", err)
			}

			slog.Info("winget source staging started", "name", srcs.winget.Name(), "dst", sourceDBPath, "purpose", "download and extract source.msix into a local SQLite database for package resolution")
			if err := retry.Do(groupCtx, cfg.Retry.Max, cfg.Retry.Backoff, func() error {
				return srcs.winget.DownloadSourceDB(groupCtx, sourceDBPath)
			}); err != nil {
				if groupCtx.Err() != nil {
					return fmt.Errorf("source %s cancelled: %w", srcs.winget.Name(), groupCtx.Err())
				}
				slog.Error("winget source staging failed", "name", srcs.winget.Name(), "dst", sourceDBPath, "elapsed", time.Since(downloadStart), "err", err)
				return fmt.Errorf("failed to stage %s source db: %w", srcs.winget.Name(), err)
			}
			slog.Info("winget source staging complete", "name", srcs.winget.Name(), "dst", sourceDBPath, "elapsed", time.Since(downloadStart))

			writeStart := time.Now()
			if err := os.MkdirAll(filepath.Dir(wingetOutPath), 0o750); err != nil {
				return fmt.Errorf("failed to create winget output dir: %w", err)
			}

			outFile, err := os.Create(wingetOutPath)
			if err != nil {
				return fmt.Errorf("failed to create winget output file: %w", err)
			}
			defer func() {
				if closeErr := outFile.Close(); err == nil && closeErr != nil {
					err = fmt.Errorf("failed to close winget output file: %w", closeErr)
				}
			}()

			writer := bufio.NewWriterSize(outFile, 256*1024)
			defer func() {
				if flushErr := writer.Flush(); err == nil && flushErr != nil {
					err = fmt.Errorf("failed to flush winget output: %w", flushErr)
				}
			}()

			slog.Info("winget package resolution started", "name", srcs.winget.Name(), "src_db", sourceDBPath, "dst", wingetOutPath, "purpose", "query the Winget index, fetch raw manifests, and write one merged JSONL stream")
			if err := srcs.winget.WriteJSONL(groupCtx, sourceDBPath, writer, cfg.Retry.Max, cfg.Retry.Backoff); err != nil {
				if groupCtx.Err() != nil {
					return fmt.Errorf("source %s cancelled: %w", srcs.winget.Name(), groupCtx.Err())
				}
				slog.Error("winget package resolution failed", "name", srcs.winget.Name(), "dst", wingetOutPath, "elapsed", time.Since(writeStart), "err", err)
				return fmt.Errorf("failed to stream packages from %s: %w", srcs.winget.Name(), err)
			}
			slog.Info("winget package resolution complete", "name", srcs.winget.Name(), "dst", wingetOutPath, "elapsed", time.Since(writeStart))

			return nil
		})
	}

	if configuredSources == 0 {
		return fmt.Errorf("no configured sources to run")
	}

	if err := group.Wait(); err != nil {
		return err
	}

	if srcs.winget != nil {
		slog.Info("pipeline complete", "sources_run", configuredSources, "winget_jsonl", wingetOutPath, "elapsed", time.Since(pipelineStart))
	} else {
		slog.Info("pipeline complete", "sources_run", configuredSources, "elapsed", time.Since(pipelineStart))
	}
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

	if srcs.scoop == nil && srcs.winget == nil {
		return crawlerSources{}, fmt.Errorf("at least one supported source must be configured")
	}

	return srcs, nil
}
