package main

import (
	"context"
	"flag"
	"log/slog"
	"os"
	"os/signal"

	"winbrew/infra/internal/crawler"
)

func main() {
	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stderr, nil)))

	configPath := flag.String("config", "config.yaml", "path to configuration file")
	wingetOutPath := flag.String("winget-out", "", "path to write the staged Winget source database")
	flag.Parse()

	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt)
	defer cancel()

	if err := crawler.Run(ctx, *configPath, *wingetOutPath); err != nil {
		slog.Error("crawler failed", "err", err)
		os.Exit(1)
	}
}
