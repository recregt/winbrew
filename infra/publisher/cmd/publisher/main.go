package main

import (
	"context"
	"errors"
	"flag"
	"log/slog"
	"os"
	"os/signal"
	"syscall"

	"winbrew/infra/publisher/internal/publisher"
)

func main() {
	slog.SetDefault(slog.New(slog.NewTextHandler(os.Stdout, nil)))

	inputPath := flag.String("input", "", "path to the catalog database to upload")
	metadataPath := flag.String("metadata", "", "path to the catalog metadata file")
	objectKey := flag.String("key", "catalog.db", "object key to use in the R2 bucket")
	updatePlansPath := flag.String("update-plans", "", "path to write D1 update plan SQL after a successful publish")
	patchChainPath := flag.String("patch-chain", "", "path to a normalized D1 patch chain manifest")
	flag.Parse()

	ctx, cancel := signal.NotifyContext(context.Background(), os.Interrupt, syscall.SIGTERM)
	defer cancel()

	if _, err := publisher.Run(ctx, *inputPath, *metadataPath, *objectKey, *updatePlansPath, *patchChainPath); err != nil {
		if errors.Is(err, context.Canceled) {
			slog.Info("publisher cancelled by user")
			os.Exit(130)
		}
		slog.Error("publisher failed", "err", err)
		os.Exit(1)
	}
}
