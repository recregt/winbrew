package crawler

import (
	"encoding/json"
	"fmt"
	"io"

	"winbrew/infra/pkg/normalize"
)

type packageSnapshot struct {
	ID          string              `json:"id"`
	Name        string              `json:"name"`
	Version     string              `json:"version"`
	Description string              `json:"description,omitempty"`
	Homepage    string              `json:"homepage,omitempty"`
	License     string              `json:"license,omitempty"`
	Publisher   string              `json:"publisher,omitempty"`
	Installers  []installerSnapshot `json:"installers,omitempty"`
}

type installerSnapshot struct {
	URL  string `json:"url"`
	Hash string `json:"hash,omitempty"`
	Arch string `json:"arch,omitempty"`
	Type string `json:"type"`
}

func emitPackagesJSONL(w io.Writer, pkgs []normalize.Package) error {
	enc := json.NewEncoder(w)

	for _, pkg := range pkgs {
		if err := enc.Encode(snapshotPackage(pkg)); err != nil {
			return fmt.Errorf("failed to encode package %s: %w", pkg.ID, err)
		}
	}

	return nil
}

func snapshotPackage(pkg normalize.Package) packageSnapshot {
	installers := make([]installerSnapshot, 0, len(pkg.Installers))
	for _, installer := range pkg.Installers {
		installers = append(installers, installerSnapshot{
			URL:  installer.URL,
			Hash: installer.Hash,
			Arch: installer.Arch,
			Type: installer.Type,
		})
	}

	return packageSnapshot{
		ID:          pkg.ID,
		Name:        pkg.Name,
		Version:     pkg.Version,
		Description: pkg.Description,
		Homepage:    pkg.Homepage,
		License:     pkg.License,
		Publisher:   pkg.Publisher,
		Installers:  installers,
	}
}
