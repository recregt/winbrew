package winget

import (
	"fmt"
	"strings"
)

type wingetPackageSnapshot struct {
	ID             string                    `json:"id"`
	Name           string                    `json:"name"`
	Version        string                    `json:"version"`
	Description    string                    `json:"description,omitempty"`
	Homepage       string                    `json:"homepage,omitempty"`
	License        string                    `json:"license,omitempty"`
	Publisher      string                    `json:"publisher,omitempty"`
	Locale         string                    `json:"locale,omitempty"`
	Moniker        string                    `json:"moniker,omitempty"`
	Platform       []string                  `json:"platform,omitempty"`
	Commands       []string                  `json:"commands,omitempty"`
	Protocols      []string                  `json:"protocols,omitempty"`
	FileExtensions []string                  `json:"file_extensions,omitempty"`
	Capabilities   []string                  `json:"capabilities,omitempty"`
	Tags           []string                  `json:"tags,omitempty"`
	Installers     []wingetInstallerSnapshot `json:"installers,omitempty"`
}

type wingetInstallerSnapshot struct {
	URL               string   `json:"url"`
	Hash              string   `json:"hash,omitempty"`
	Arch              string   `json:"arch,omitempty"`
	Type              string   `json:"type"`
	Commands          []string `json:"commands,omitempty"`
	Protocols         []string `json:"protocols,omitempty"`
	FileExtensions    []string `json:"file_extensions,omitempty"`
	Capabilities      []string `json:"capabilities,omitempty"`
	Platform          []string `json:"platform,omitempty"`
	NestedKind        string   `json:"NestedInstallerType,omitempty"`
	Scope             string   `json:"scope,omitempty"`
	InstallerSwitches string   `json:"installer_switches,omitempty"`
}

func firstNonEmpty(values ...string) string {
	for _, value := range values {
		if trimmed := strings.TrimSpace(value); trimmed != "" {
			return trimmed
		}
	}

	return ""
}

func firstNonEmptyStrings[T ~[]string](values ...T) []string {
	for _, value := range values {
		if len(value) == 0 {
			continue
		}

		result := make([]string, 0, len(value))
		for _, item := range value {
			if trimmed := strings.TrimSpace(item); trimmed != "" {
				result = append(result, trimmed)
			}
		}
		if len(result) > 0 {
			return result
		}
	}

	return nil
}

func ensureWingetPackageCoordinate(expected, actual string) error {
	if trimmed := strings.TrimSpace(actual); trimmed != "" && trimmed != expected {
		return fmt.Errorf("winget manifest identifier mismatch: expected %s, got %s", expected, trimmed)
	}

	return nil
}

func buildWingetPackageSnapshot(row wingetIndexRow, rootManifest wingetManifest, localeManifest, installerManifest *wingetManifest) (wingetPackageSnapshot, error) {
	if err := ensureWingetPackageCoordinate(row.id, rootManifest.PackageIdentifier); err != nil {
		return wingetPackageSnapshot{}, err
	}

	packageType := strings.ToLower(strings.TrimSpace(rootManifest.ManifestType))
	switch packageType {
	case "singleton":
		installers, err := rootManifest.resolveInstallers()
		if err != nil {
			return wingetPackageSnapshot{}, fmt.Errorf("failed to resolve winget installers for %s: %w", row.id, err)
		}

		return wingetPackageSnapshot{
			ID:             "winget/" + row.id,
			Name:           firstNonEmpty(rootManifest.PackageName, row.name),
			Version:        firstNonEmpty(rootManifest.PackageVersion, row.version),
			Description:    firstNonEmpty(rootManifest.ShortDescription, rootManifest.Description),
			Homepage:       strings.TrimSpace(rootManifest.Homepage),
			License:        strings.TrimSpace(rootManifest.License),
			Publisher:      firstNonEmpty(rootManifest.Publisher, row.publisher),
			Locale:         firstNonEmpty(rootManifest.PackageLocale, rootManifest.DefaultLocale),
			Moniker:        firstNonEmpty(rootManifest.Moniker),
			Platform:       firstNonEmptyStrings(rootManifest.Platform),
			Commands:       firstNonEmptyStrings(rootManifest.Commands),
			Protocols:      firstNonEmptyStrings(rootManifest.Protocols),
			FileExtensions: firstNonEmptyStrings(rootManifest.FileExtensions),
			Capabilities:   firstNonEmptyStrings(rootManifest.Capabilities),
			Tags:           firstNonEmptyStrings(rootManifest.Tags),
			Installers:     installers,
		}, nil
	case "version":
		if localeManifest == nil {
			return wingetPackageSnapshot{}, fmt.Errorf("winget package %s is missing locale manifest", row.id)
		}
		if installerManifest == nil {
			return wingetPackageSnapshot{}, fmt.Errorf("winget package %s is missing installer manifest", row.id)
		}

		if err := ensureWingetPackageCoordinate(row.id, localeManifest.PackageIdentifier); err != nil {
			return wingetPackageSnapshot{}, err
		}
		if err := ensureWingetPackageCoordinate(row.id, installerManifest.PackageIdentifier); err != nil {
			return wingetPackageSnapshot{}, err
		}

		installers, err := installerManifest.resolveInstallers()
		if err != nil {
			return wingetPackageSnapshot{}, fmt.Errorf("failed to resolve winget installers for %s: %w", row.id, err)
		}

		return wingetPackageSnapshot{
			ID:             "winget/" + row.id,
			Name:           firstNonEmpty(localeManifest.PackageName, rootManifest.PackageName, row.name),
			Version:        firstNonEmpty(rootManifest.PackageVersion, row.version),
			Description:    firstNonEmpty(localeManifest.ShortDescription, localeManifest.Description, rootManifest.ShortDescription, rootManifest.Description),
			Homepage:       firstNonEmpty(localeManifest.Homepage, rootManifest.Homepage),
			License:        firstNonEmpty(localeManifest.License, rootManifest.License),
			Publisher:      firstNonEmpty(localeManifest.Publisher, rootManifest.Publisher, row.publisher),
			Locale:         firstNonEmpty(localeManifest.PackageLocale, rootManifest.PackageLocale, rootManifest.DefaultLocale),
			Moniker:        firstNonEmpty(localeManifest.Moniker, rootManifest.Moniker),
			Platform:       firstNonEmptyStrings(localeManifest.Platform, rootManifest.Platform, installerManifest.Platform),
			Commands:       firstNonEmptyStrings(localeManifest.Commands, rootManifest.Commands, installerManifest.Commands),
			Protocols:      firstNonEmptyStrings(localeManifest.Protocols, rootManifest.Protocols, installerManifest.Protocols),
			FileExtensions: firstNonEmptyStrings(localeManifest.FileExtensions, rootManifest.FileExtensions, installerManifest.FileExtensions),
			Capabilities:   firstNonEmptyStrings(localeManifest.Capabilities, rootManifest.Capabilities, installerManifest.Capabilities),
			Tags:           firstNonEmptyStrings(localeManifest.Tags, rootManifest.Tags),
			Installers:     installers,
		}, nil
	default:
		return wingetPackageSnapshot{}, fmt.Errorf("unsupported winget manifest type %q for %s", rootManifest.ManifestType, row.id)
	}
}
