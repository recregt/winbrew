package main

import (
	"context"
	"flag"
	"log/slog"
	"os"

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

	if _, err := publisher.Run(context.Background(), *inputPath, *metadataPath, *objectKey, *updatePlansPath, *patchChainPath); err != nil {
		slog.Error("publisher failed", "err", err)
		os.Exit(1)
	}
}
