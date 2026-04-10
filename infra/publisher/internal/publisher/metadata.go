package publisher

import (
	"encoding/json"
	"fmt"
	"os"
	"path"
	"path/filepath"
)

const metadataSchemaVersion = 1

type Metadata struct {
	SchemaVersion   uint32         `json:"schema_version"`
	GeneratedAtUnix uint64         `json:"generated_at_unix"`
	CurrentHash     string         `json:"current_hash"`
	PreviousHash    string         `json:"previous_hash,omitempty"`
	PackageCount    int            `json:"package_count"`
	SourceCounts    map[string]int `json:"source_counts"`
}

func LoadMetadata(path string) (Metadata, error) {
	file, err := os.Open(path)
	if err != nil {
		return Metadata{}, fmt.Errorf("failed to read metadata file: %w", err)
	}
	defer file.Close()

	var metadata Metadata
	if err := json.NewDecoder(file).Decode(&metadata); err != nil {
		return Metadata{}, fmt.Errorf("failed to decode metadata file: %w", err)
	}
	if err := metadata.validate(); err != nil {
		return Metadata{}, err
	}

	if metadata.SchemaVersion != metadataSchemaVersion {
		return Metadata{}, fmt.Errorf("unsupported metadata schema version: %d", metadata.SchemaVersion)
	}

	return metadata, nil
}

func SaveMetadata(path string, metadata Metadata) error {
	data, err := metadataBytes(metadata)
	if err != nil {
		return err
	}

	if err := os.MkdirAll(filepath.Dir(path), 0o750); err != nil {
		return fmt.Errorf("failed to create metadata directory: %w", err)
	}

	return writeFileAtomic(path, data, 0o644)
}

func metadataBytes(metadata Metadata) ([]byte, error) {
	if err := metadata.validate(); err != nil {
		return nil, err
	}
	if metadata.SchemaVersion != metadataSchemaVersion {
		return nil, fmt.Errorf("unsupported metadata schema version: %d", metadata.SchemaVersion)
	}

	data, err := json.MarshalIndent(metadata, "", "  ")
	if err != nil {
		return nil, fmt.Errorf("failed to encode metadata: %w", err)
	}

	return append(data, '\n'), nil
}

func writeFileAtomic(path string, data []byte, perm os.FileMode) error {
	tempFile, err := os.CreateTemp(filepath.Dir(path), filepath.Base(path)+".*.tmp")
	if err != nil {
		return fmt.Errorf("failed to create metadata temp file: %w", err)
	}
	tempPath := tempFile.Name()
	renamed := false
	defer func() {
		if !renamed {
			_ = tempFile.Close()
			_ = os.Remove(tempPath)
		}
	}()

	if _, err := tempFile.Write(data); err != nil {
		return fmt.Errorf("failed to write metadata temp file: %w", err)
	}
	if err := tempFile.Chmod(perm); err != nil {
		return fmt.Errorf("failed to set metadata file permissions: %w", err)
	}
	if err := tempFile.Close(); err != nil {
		return fmt.Errorf("failed to close metadata temp file: %w", err)
	}
	if err := os.Rename(tempPath, path); err != nil {
		return fmt.Errorf("failed to replace metadata file: %w", err)
	}
	renamed = true

	return nil
}

func metadataKeyForObjectKey(objectKey string) string {
	// Object keys use slash separators regardless of host OS.
	return path.Join(path.Dir(objectKey), "metadata.json")
}

func (m *Metadata) validate() error {
	if m == nil {
		return fmt.Errorf("metadata cannot be nil")
	}
	if m.CurrentHash == "" {
		return fmt.Errorf("metadata.current_hash cannot be empty")
	}
	if m.SourceCounts == nil {
		m.SourceCounts = map[string]int{}
	}
	return nil
}
