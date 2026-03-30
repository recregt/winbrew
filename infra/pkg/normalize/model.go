package normalize

import "encoding/json"

type Package struct {
	// Required
	ID      string // "winget/Microsoft.VSCode" | "scoop/vscode"
	Name    string
	Version string
	Source  string // "winget" | "scoop"

	// Optional
	Description string
	Homepage    string
	License     string
	Publisher   string

	Installers []Installer

	Raw json.RawMessage
}

type Installer struct {
	URL  string
	Hash string
	Arch string // x64, x86, arm64
	Type string // msi, msix, exe, portable, zip
}
