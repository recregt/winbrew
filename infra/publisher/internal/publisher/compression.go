package publisher

import (
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"

	"github.com/klauspost/compress/zstd"
)

func compressSnapshotToTemp(inputPath string) (string, int64, error) {
	inputFile, err := os.Open(inputPath)
	if err != nil {
		return "", 0, fmt.Errorf("failed to open catalog snapshot for compression: %w", err)
	}
	defer inputFile.Close()

	return compressReaderToTemp(inputFile, filepath.Base(inputPath)+".*.zst")

}

func compressTextToTemp(text string, tempNamePattern string) (string, int64, error) {
	return compressReaderToTemp(strings.NewReader(text), tempNamePattern)
}

func decompressSnapshotToTemp(compressedPath string) (string, error) {
	compressedFile, err := os.Open(compressedPath)
	if err != nil {
		return "", fmt.Errorf("failed to open compressed snapshot: %w", err)
	}
	defer compressedFile.Close()

	outputFile, err := os.CreateTemp("", filepath.Base(compressedPath)+".*.db")
	if err != nil {
		return "", fmt.Errorf("failed to create decompressed snapshot temp file: %w", err)
	}
	outputPath := outputFile.Name()
	committed := false
	defer func() {
		if !committed {
			_ = outputFile.Close()
			_ = os.Remove(outputPath)
		}
	}()

	decoder, err := zstd.NewReader(compressedFile)
	if err != nil {
		return "", fmt.Errorf("failed to create zstd decoder: %w", err)
	}
	defer decoder.Close()

	if _, err := io.Copy(outputFile, decoder); err != nil {
		return "", fmt.Errorf("failed to decompress snapshot: %w", err)
	}
	if err := outputFile.Sync(); err != nil {
		return "", fmt.Errorf("failed to sync decompressed snapshot: %w", err)
	}
	if err := outputFile.Close(); err != nil {
		return "", fmt.Errorf("failed to close decompressed snapshot: %w", err)
	}

	committed = true
	return outputPath, nil
}

func compressReaderToTemp(reader io.Reader, tempNamePattern string) (string, int64, error) {
	tempFile, err := os.CreateTemp("", tempNamePattern)
	if err != nil {
		return "", 0, fmt.Errorf("failed to create compressed snapshot temp file: %w", err)
	}
	tempPath := tempFile.Name()
	committed := false
	defer func() {
		if !committed {
			_ = tempFile.Close()
			_ = os.Remove(tempPath)
		}
	}()

	encoder, err := zstd.NewWriter(tempFile, zstd.WithEncoderLevel(zstd.SpeedDefault))
	if err != nil {
		return "", 0, fmt.Errorf("failed to create zstd encoder: %w", err)
	}

	if _, err := io.Copy(encoder, reader); err != nil {
		_ = encoder.Close()
		return "", 0, fmt.Errorf("failed to compress catalog snapshot: %w", err)
	}
	if err := encoder.Close(); err != nil {
		return "", 0, fmt.Errorf("failed to finish zstd compression: %w", err)
	}
	if err := tempFile.Sync(); err != nil {
		return "", 0, fmt.Errorf("failed to sync compressed snapshot: %w", err)
	}
	if err := tempFile.Close(); err != nil {
		return "", 0, fmt.Errorf("failed to close compressed snapshot temp file: %w", err)
	}

	info, err := os.Stat(tempPath)
	if err != nil {
		return "", 0, fmt.Errorf("failed to inspect compressed snapshot size: %w", err)
	}

	committed = true
	return tempPath, info.Size(), nil
}
