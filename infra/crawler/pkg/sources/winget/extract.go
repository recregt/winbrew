package winget

import (
	"archive/zip"
	"fmt"
	"io"
	"os"
	"path"
	"path/filepath"
	"strings"
)

const maxIndexDBSize = 500 << 20

func extractDB(msixPath, dstPath string) (string, error) {
	r, err := zip.OpenReader(msixPath)
	if err != nil {
		return "", fmt.Errorf("failed to open msix: %w", err)
	}
	defer r.Close()

	var found bool

	for _, f := range r.File {
		if !isIndexDBEntry(f.Name) {
			continue
		}
		if found {
			return "", fmt.Errorf("multiple index.db entries found in msix")
		}
		found = true

		if err := extractFile(f, dstPath); err != nil {
			return "", fmt.Errorf("failed to extract %s: %w", f.Name, err)
		}
	}

	if !found {
		return "", fmt.Errorf("index.db not found in msix")
	}

	return dstPath, nil
}

func extractFile(f *zip.File, dst string) (err error) {
	if isTraversalEntry(f.Name) {
		return fmt.Errorf("invalid zip entry with path traversal: %s", f.Name)
	}
	if f.UncompressedSize64 > maxIndexDBSize {
		return fmt.Errorf("zip entry too large: %d bytes", f.UncompressedSize64)
	}

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
	defer func() {
		if err != nil {
			_ = os.Remove(tempPath)
		}
	}()

	buf := make([]byte, 32*1024)
	if _, err = io.CopyBuffer(out, rc, buf); err != nil {
		return fmt.Errorf("failed to extract file: %w", err)
	}

	if err = out.Close(); err != nil {
		return fmt.Errorf("failed to close temp file: %w", err)
	}

	if err = os.Rename(tempPath, dst); err != nil {
		return fmt.Errorf("failed to move extracted file into place: %w", err)
	}

	return nil
}

func isIndexDBEntry(name string) bool {
	normalized := strings.ToLower(filepath.ToSlash(name))
	return normalized == "public/index.db"
}

func isTraversalEntry(name string) bool {
	normalized := filepath.ToSlash(name)
	cleaned := path.Clean("/" + normalized)
	return cleaned != "/"+normalized
}
