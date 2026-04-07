package main

import (
	"context"
	"errors"
	"flag"
	"log/slog"
	"os"
	"os/signal"
	"syscall"

	"infra/crawler/internal/crawler"
)

func main() {
	defer func() {
		if recovered := recover(); recovered != nil {
			slog.Error("unexpected panic", "recover", recovered)
			os.Exit(1)
		}
	}()

	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, nil)))

	configPath := flag.String("config", "config.yaml", "path to configuration file")
	wingetOutPath := flag.String("winget-out", "", "path to write the staged Winget source database")
	flag.Parse()

	if *configPath == "" {
		slog.Error("config path is required")
		flag.Usage()
		os.Exit(2)
	}

	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()

	if err := crawler.Run(ctx, *configPath, *wingetOutPath); err != nil {
		if errors.Is(err, context.Canceled) {
			slog.Info("crawler cancelled by user")
			os.Exit(130)
		}
		slog.Error("crawler failed", "err", err)
		os.Exit(1)
	}
}
