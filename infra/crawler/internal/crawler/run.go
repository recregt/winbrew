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

	return runPipeline(ctx, cfg, srcs, cacheDir, trimmed)
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
	group, groupCtx := errgroup.WithContext(ctx)
	configuredSources := 0

	if srcs.scoop != nil {
		configuredSources++
		group.Go(func() (err error) {
			slog.Info("streaming source", "name", srcs.scoop.Name())
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
				return fmt.Errorf("failed to stream packages from %s: %w", srcs.scoop.Name(), err)
			}

			return nil
		})
	}

	if srcs.winget != nil {
		configuredSources++
		group.Go(func() error {
			sourceDBPath := filepath.Join(cacheDir, "winget", "winget_source.db")
			if err := os.MkdirAll(filepath.Dir(sourceDBPath), 0o750); err != nil {
				return fmt.Errorf("failed to create winget cache dir: %w", err)
			}

			slog.Info("downloading winget source db", "name", srcs.winget.Name(), "dst", sourceDBPath)
			if err := retry.Do(groupCtx, cfg.Retry.Max, cfg.Retry.Backoff, func() error {
				return srcs.winget.DownloadSourceDB(groupCtx, sourceDBPath)
			}); err != nil {
				if groupCtx.Err() != nil {
					return fmt.Errorf("source %s cancelled: %w", srcs.winget.Name(), groupCtx.Err())
				}
				return fmt.Errorf("failed to stage %s source db: %w", srcs.winget.Name(), err)
			}

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

			slog.Info("writing winget JSONL", "name", srcs.winget.Name(), "dst", wingetOutPath)
			if err := srcs.winget.WriteJSONL(groupCtx, sourceDBPath, writer, cfg.Retry.Max, cfg.Retry.Backoff); err != nil {
				if groupCtx.Err() != nil {
					return fmt.Errorf("source %s cancelled: %w", srcs.winget.Name(), groupCtx.Err())
				}
				return fmt.Errorf("failed to stream packages from %s: %w", srcs.winget.Name(), err)
			}

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
		slog.Info("pipeline complete", "sources_run", configuredSources, "winget_jsonl", wingetOutPath)
	} else {
		slog.Info("pipeline complete", "sources_run", configuredSources)
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
