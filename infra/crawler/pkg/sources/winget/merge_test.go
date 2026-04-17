package winget

import (
	"strings"
	"testing"
)

func TestWingetManifestResolutionSingleton(t *testing.T) {
	t.Parallel()

	manifestYAML := strings.Join([]string{
		"PackageIdentifier: Microsoft.WindowsTerminal",
		"PackageVersion: 1.9.1942.0",
		"PackageLocale: en-US",
		"PackageName: Windows Terminal",
		"Publisher: Microsoft Corporation",
		"Moniker: wt",
		"Platform: Windows.Desktop",
		"Tags:",
		"  - terminal",
		"  - shell",
		"ShortDescription: Modern terminal",
		"License: MIT",
		"Homepage: https://example.invalid",
		"Installers:",
		"  - Architecture: x64",
		"    InstallerType: msix",
		"    InstallerUrl: https://example.invalid/terminal.msixbundle",
		"    InstallerSha256: ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789",
		"    Scope: user",
		"    InstallerSwitches:",
		"      SilentWithProgress: /terminal-default",
		"  - Architecture: arm",
		"    InstallerType: zip",
		"    InstallerUrl: https://example.invalid/terminal.zip",
		"    InstallerSha256: 0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
		"    NestedInstallerType: portable",
		"ManifestType: singleton",
		"ManifestVersion: 1.12.0",
	}, "\n")

	manifest, err := parseWingetManifest([]byte(manifestYAML))
	if err != nil {
		t.Fatalf("parseWingetManifest() error = %v", err)
	}
	if got, want := len(manifest.Platform), 1; got != want {
		t.Fatalf("manifest platform length = %d, want %d", got, want)
	}
	if got, want := manifest.Platform[0], "Windows.Desktop"; got != want {
		t.Fatalf("manifest platform[0] = %q, want %q", got, want)
	}

	pkg, err := buildWingetPackageSnapshot(wingetIndexRow{
		id:        "Microsoft.WindowsTerminal",
		name:      "Windows Terminal",
		version:   "1.9.1942.0",
		publisher: "Microsoft Corporation",
	}, manifest, nil, nil)
	if err != nil {
		t.Fatalf("buildWingetPackageSnapshot() error = %v", err)
	}

	if got, want := pkg.ID, "winget/Microsoft.WindowsTerminal"; got != want {
		t.Fatalf("package id = %q, want %q", got, want)
	}
	if got, want := pkg.Name, "Windows Terminal"; got != want {
		t.Fatalf("package name = %q, want %q", got, want)
	}
	if got, want := pkg.Publisher, "Microsoft Corporation"; got != want {
		t.Fatalf("package publisher = %q, want %q", got, want)
	}
	if got, want := pkg.Locale, "en-US"; got != want {
		t.Fatalf("package locale = %q, want %q", got, want)
	}
	if got, want := pkg.Moniker, "wt"; got != want {
		t.Fatalf("package moniker = %q, want %q", got, want)
	}
	if got, want := len(pkg.Tags), 2; got != want {
		t.Fatalf("package tags length = %d, want %d", got, want)
	}
	if got, want := pkg.Installers[0].Scope, "user"; got != want {
		t.Fatalf("installer scope = %q, want %q", got, want)
	}
	if got, want := pkg.Installers[0].InstallerSwitches, "/terminal-default"; got != want {
		t.Fatalf("installer switches = %q, want %q", got, want)
	}
	if got, want := pkg.Installers[1].Arch, ""; got != want {
		t.Fatalf("installer arch = %q, want %q", got, want)
	}
	if got, want := pkg.Installers[1].NestedKind, "portable"; got != want {
		t.Fatalf("installer nested kind = %q, want %q", got, want)
	}
}

func TestWingetManifestResolutionMultiFile(t *testing.T) {
	t.Parallel()

	root, err := parseWingetManifest([]byte(`
PackageIdentifier: Contoso.App
PackageVersion: 2.3.4
DefaultLocale: en-US
Moniker: contoso-app
Tags:
  - utility
ManifestType: version
ManifestVersion: 1.12.0
`))
	if err != nil {
		t.Fatalf("parseWingetManifest(root) error = %v", err)
	}

	locale, err := parseWingetManifest([]byte(`
PackageIdentifier: Contoso.App
PackageVersion: 2.3.4
PackageLocale: en-US
Publisher: Contoso Ltd.
PackageName: Contoso App
Moniker: contoso
Tags:
  - editor
  - productivity
ShortDescription: Contoso app
Homepage: https://contoso.example
License: MIT
ManifestType: defaultLocale
ManifestVersion: 1.12.0
`))
	if err != nil {
		t.Fatalf("parseWingetManifest(locale) error = %v", err)
	}

	installerYAML := strings.Join([]string{
		"PackageIdentifier: Contoso.App",
		"PackageVersion: 2.3.4",
		"Installers:",
		"  - Architecture: x64",
		"    Platform:",
		"      - Windows.Desktop",
		"    InstallerType: exe",
		"    InstallerUrl: https://example.invalid/app.exe",
		"    InstallerSha256: 0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF",
		"    Scope: machine",
		"    InstallerSwitches:",
		"      SilentWithProgress: /app-installer",
		"ManifestType: installer",
		"ManifestVersion: 1.12.0",
	}, "\n")

	installer, err := parseWingetManifest([]byte(installerYAML))
	if err != nil {
		t.Fatalf("parseWingetManifest(installer) error = %v", err)
	}
	if got, want := len(installer.Installers[0].Platform), 1; got != want {
		t.Fatalf("installer platform length = %d, want %d", got, want)
	}
	if got, want := installer.Installers[0].Platform[0], "Windows.Desktop"; got != want {
		t.Fatalf("installer platform[0] = %q, want %q", got, want)
	}

	pkg, err := buildWingetPackageSnapshot(wingetIndexRow{
		id:        "Contoso.App",
		name:      "Contoso App",
		version:   "2.3.4",
		publisher: "Contoso Ltd.",
	}, root, &locale, &installer)
	if err != nil {
		t.Fatalf("buildWingetPackageSnapshot() error = %v", err)
	}

	if got, want := pkg.Description, "Contoso app"; got != want {
		t.Fatalf("package description = %q, want %q", got, want)
	}
	if got, want := pkg.Locale, "en-US"; got != want {
		t.Fatalf("package locale = %q, want %q", got, want)
	}
	if got, want := pkg.Moniker, "contoso"; got != want {
		t.Fatalf("package moniker = %q, want %q", got, want)
	}
	if got, want := len(pkg.Tags), 2; got != want {
		t.Fatalf("package tags length = %d, want %d", got, want)
	}
	if got, want := pkg.Installers[0].Scope, "machine"; got != want {
		t.Fatalf("installer scope = %q, want %q", got, want)
	}
	if got, want := pkg.Installers[0].InstallerSwitches, "/app-installer"; got != want {
		t.Fatalf("installer switches = %q, want %q", got, want)
	}
	if got, want := pkg.Installers[0].Arch, "x64"; got != want {
		t.Fatalf("installer arch = %q, want %q", got, want)
	}
	if got, want := pkg.Installers[0].Type, "exe"; got != want {
		t.Fatalf("installer type = %q, want %q", got, want)
	}
}
