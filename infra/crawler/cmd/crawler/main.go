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

	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()

	exitOnErr := func(err error) {
		if errors.Is(err, context.Canceled) {
			slog.Info("crawler cancelled by user")
			os.Exit(130)
		}
		slog.Error("crawler failed", "err", err)
		os.Exit(1)
	}

	if len(os.Args) > 1 && os.Args[1] == "tools" {
		if err := runTools(ctx, os.Args[2:]); err != nil {
			exitOnErr(err)
		}
		return
	}

	configPath := flag.String("config", "config.yaml", "path to configuration file")
	wingetOutPath := flag.String("winget-out", "", "path to write the Winget JSONL output file")
	flag.Parse()

	if *configPath == "" {
		slog.Error("config path is required")
		flag.Usage()
		os.Exit(2)
	}

	if err := crawler.Run(ctx, *configPath, *wingetOutPath); err != nil {
		exitOnErr(err)
	}
}
