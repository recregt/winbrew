package publisher

import (
	"bytes"
	"encoding/json"
	"errors"
	"fmt"
	"io/fs"
	"os"
	"sort"
	"strings"
)

const maxPatchChainLength = 7

type patchChainArtifact struct {
	Depth           int    `json:"depth"`
	FilePath        string `json:"file_path"`
	SizeBytes       int64  `json:"size_bytes"`
	ReachedPrevious bool   `json:"reached_previous"`
}

func loadPatchChain(path string) ([]patchChainArtifact, error) {
	path = strings.TrimSpace(path)
	if path == "" {
		return nil, nil
	}

	data, err := os.ReadFile(path)
	if err != nil {
		if errors.Is(err, fs.ErrNotExist) {
			return nil, nil
		}

		return nil, fmt.Errorf("failed to read patch chain manifest: %w", err)
	}

	trimmed := bytes.TrimSpace(data)
	if len(trimmed) == 0 || bytes.Equal(trimmed, []byte("[]")) {
		return nil, nil
	}

	var artifacts []patchChainArtifact
	if err := json.Unmarshal(trimmed, &artifacts); err != nil {
		return nil, fmt.Errorf("failed to decode patch chain manifest: %w", err)
	}

	return artifacts, nil
}

func buildPatchChainRow(publicBaseURL, currentHash, previousHash string, artifacts []patchChainArtifact, fullSnapshotBytes int64) (updatePlanSQLRow, bool, error) {
	if len(artifacts) == 0 {
		return updatePlanSQLRow{}, false, nil
	}

	sort.SliceStable(artifacts, func(i, j int) bool {
		if artifacts[i].Depth == artifacts[j].Depth {
			return artifacts[i].FilePath < artifacts[j].FilePath
		}

		return artifacts[i].Depth > artifacts[j].Depth
	})

	if !artifacts[0].ReachedPrevious {
		return updatePlanSQLRow{}, false, nil
	}
	if fullSnapshotBytes <= 0 {
		return updatePlanSQLRow{}, false, fmt.Errorf("full snapshot size must be greater than zero")
	}
	if len(artifacts) > maxPatchChainLength {
		return updatePlanSQLRow{}, false, nil
	}

	var totalPatchBytes int64
	for _, artifact := range artifacts {
		if artifact.SizeBytes <= 0 {
			return updatePlanSQLRow{}, false, fmt.Errorf("patch chain artifact size_bytes cannot be empty")
		}
		if artifact.SizeBytes*100 > fullSnapshotBytes*40 {
			return updatePlanSQLRow{}, false, nil
		}
		totalPatchBytes += artifact.SizeBytes
	}

	patchURLs := make([]string, 0, len(artifacts))
	for _, artifact := range artifacts {
		if strings.TrimSpace(artifact.FilePath) == "" {
			return updatePlanSQLRow{}, false, fmt.Errorf("patch chain artifact file path cannot be empty")
		}

		patchURL, err := publicObjectURL(publicBaseURL, artifact.FilePath)
		if err != nil {
			return updatePlanSQLRow{}, false, err
		}

		patchURLs = append(patchURLs, patchURL)
	}

	if len(patchURLs) == 0 {
		return updatePlanSQLRow{}, false, nil
	}

	patchURLsJSON, err := json.Marshal(patchURLs)
	if err != nil {
		return updatePlanSQLRow{}, false, fmt.Errorf("failed to encode patch URLs: %w", err)
	}

	return updatePlanSQLRow{
		currentHash:     previousHash,
		mode:            "patch",
		targetHash:      currentHash,
		snapshotURL:     "",
		patchURLsJSON:   string(patchURLsJSON),
		chainLength:     len(patchURLs),
		totalPatchBytes: totalPatchBytes,
		isLatestFull:    0,
		isStale:         0,
	}, true, nil
}
