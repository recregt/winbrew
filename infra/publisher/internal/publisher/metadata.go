package publisher

import (
	"encoding/json"
	"fmt"
	"os"
	"path"
	"path/filepath"
)

type Metadata struct {
	SchemaVersion   uint32         `json:"schema_version"`
	GeneratedAtUnix uint64         `json:"generated_at_unix"`
	CurrentHash     string         `json:"current_hash"`
	PreviousHash    string         `json:"previous_hash,omitempty"`
	PackageCount    int            `json:"package_count"`
	SourceCounts    map[string]int `json:"source_counts"`
}

func LoadMetadata(path string) (Metadata, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return Metadata{}, fmt.Errorf("failed to read metadata file: %w", err)
	}

	var metadata Metadata
	if err := json.Unmarshal(data, &metadata); err != nil {
		return Metadata{}, fmt.Errorf("failed to decode metadata file: %w", err)
	}

	if metadata.CurrentHash == "" {
		return Metadata{}, fmt.Errorf("metadata.current_hash cannot be empty")
	}
	if metadata.SourceCounts == nil {
		metadata.SourceCounts = map[string]int{}
	}

	return metadata, nil
}

func SaveMetadata(path string, metadata Metadata) error {
	if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
		return fmt.Errorf("failed to create metadata directory: %w", err)
	}

	data, err := json.MarshalIndent(metadata, "", "  ")
	if err != nil {
		return fmt.Errorf("failed to encode metadata: %w", err)
	}
	data = append(data, '\n')

	if err := os.WriteFile(path, data, 0o644); err != nil {
		return fmt.Errorf("failed to write metadata file: %w", err)
	}

	return nil
}

func metadataKeyForObjectKey(objectKey string) string {
	return path.Join(path.Dir(objectKey), "metadata.json")
}
