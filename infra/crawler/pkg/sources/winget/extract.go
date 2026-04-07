package winget

import (
	"archive/zip"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"strings"
)

func extractDB(msixPath, dstPath string) (string, error) {
	r, err := zip.OpenReader(msixPath)
	if err != nil {
		return "", fmt.Errorf("failed to open msix: %w", err)
	}
	defer r.Close()

	for _, f := range r.File {
		if !strings.EqualFold(f.Name, "Public/index.db") {
			continue
		}

		if err := extractFile(f, dstPath); err != nil {
			return "", err
		}
		return dstPath, nil
	}

	return "", fmt.Errorf("index.db not found in msix")
}

func extractFile(f *zip.File, dst string) error {
	rc, err := f.Open()
	if err != nil {
		return fmt.Errorf("failed to open zip entry: %w", err)
	}
	defer rc.Close()

	out, err := os.CreateTemp(filepath.Dir(dst), filepath.Base(dst)+".*.tmp")
	if err != nil {
		return fmt.Errorf("failed to create temp file: %w", err)
	}
	tempPath := out.Name()

	if _, err := io.Copy(out, rc); err != nil {
		_ = out.Close()
		_ = os.Remove(tempPath)
		return fmt.Errorf("failed to extract file: %w", err)
	}

	if err := out.Close(); err != nil {
		_ = os.Remove(tempPath)
		return fmt.Errorf("failed to close temp file: %w", err)
	}

	if err := os.Rename(tempPath, dst); err != nil {
		_ = os.Remove(tempPath)
		return fmt.Errorf("failed to move extracted file into place: %w", err)
	}

	return nil
}
