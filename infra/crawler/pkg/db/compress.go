package db

import (
	"fmt"
	"io"
	"os"
	"path/filepath"

	"github.com/klauspost/compress/zstd"
)

func CompressFile(src, dst string, level zstd.EncoderLevel) error {
	in, err := os.Open(src)
	if err != nil {
		return fmt.Errorf("failed to open source file: %w", err)
	}
	defer in.Close()

	out, err := os.CreateTemp(filepath.Dir(dst), filepath.Base(dst)+".*.tmp")
	if err != nil {
		return fmt.Errorf("failed to create destination file: %w", err)
	}
	tempPath := out.Name()

	enc, err := zstd.NewWriter(out, zstd.WithEncoderLevel(level))
	if err != nil {
		_ = out.Close()
		_ = os.Remove(tempPath)
		return fmt.Errorf("failed to create zstd encoder: %w", err)
	}

	if _, err := io.Copy(enc, in); err != nil {
		_ = enc.Close()
		_ = out.Close()
		_ = os.Remove(tempPath)
		return fmt.Errorf("failed to compress: %w", err)
	}

	if err := enc.Close(); err != nil {
		_ = out.Close()
		_ = os.Remove(tempPath)
		return fmt.Errorf("failed to flush zstd encoder: %w", err)
	}

	if err := out.Close(); err != nil {
		_ = os.Remove(tempPath)
		return fmt.Errorf("failed to close temporary file: %w", err)
	}

	if err := os.Rename(tempPath, dst); err != nil {
		_ = os.Remove(tempPath)
		return fmt.Errorf("failed to replace destination file: %w", err)
	}

	return nil
}
