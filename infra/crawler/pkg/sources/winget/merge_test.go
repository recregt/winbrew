package winget

import (
	"testing"
)

func TestWingetManifestResolutionSingleton(t *testing.T) {
	t.Parallel()

	manifest, err := parseWingetManifest([]byte(`
PackageIdentifier: Microsoft.WindowsTerminal
PackageVersion: 1.9.1942.0
PackageLocale: en-US
PackageName: Windows Terminal
Publisher: Microsoft Corporation
Moniker: wt
Tags:
  - terminal
  - shell
ShortDescription: Modern terminal
License: MIT
Homepage: https://example.invalid
Installers:
  - Architecture: x64
    InstallerType: msix
    InstallerUrl: https://example.invalid/terminal.msixbundle
    InstallerSha256: ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789
    Scope: user
  - Architecture: arm
    InstallerType: zip
    InstallerUrl: https://example.invalid/terminal.zip
    InstallerSha256: 0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF
    NestedInstallerType: portable
ManifestType: singleton
ManifestVersion: 1.12.0
`))
	if err != nil {
		t.Fatalf("parseWingetManifest() error = %v", err)
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

	installer, err := parseWingetManifest([]byte(`
PackageIdentifier: Contoso.App
PackageVersion: 2.3.4
Installers:
  - Architecture: x64
    InstallerType: exe
    InstallerUrl: https://example.invalid/app.exe
    InstallerSha256: 0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF
    Scope: machine
ManifestType: installer
ManifestVersion: 1.12.0
`))
	if err != nil {
		t.Fatalf("parseWingetManifest(installer) error = %v", err)
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
	if got, want := pkg.Installers[0].Arch, "x64"; got != want {
		t.Fatalf("installer arch = %q, want %q", got, want)
	}
	if got, want := pkg.Installers[0].Type, "exe"; got != want {
		t.Fatalf("installer type = %q, want %q", got, want)
	}
}
