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
	flag.Parse()

	if err := publisher.Run(context.Background(), *inputPath, *metadataPath, *objectKey); err != nil {
		slog.Error("publisher failed", "err", err)
		os.Exit(1)
	}
}
