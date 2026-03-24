# WinBrew

[![CI](https://github.com/recregt/winbrew/actions/workflows/ci.yml/badge.svg)](https://github.com/recregt/winbrew/actions)
[![Release](https://github.com/recregt/winbrew/actions/workflows/release.yml/badge.svg)](https://github.com/recregt/winbrew/actions)
[![Version](https://img.shields.io/github/v/release/recregt/winbrew?include_prereleases&color=blue&logo=github&label=Version)](https://github.com/recregt/winbrew/releases/latest)

A modern package manager for Windows that installs, tracks, and cleanly removes software.

> [!WARNING]
> This project is in the early stages of development.

## Requirements

- Windows 10 or later

## Installation

Just copy and paste this command to Powershell (Admin):

```powershell
PowerShell -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/recregt/winbrew/main/scripts/install.ps1 | iex"
```

### Installed layout

By default, `brew` uses `C:\winbrew` as its root directory:

```text
C:\winbrew
├── packages
└── data
    ├── winbrew.toml
    ├── db
    │   └── winbrew.db
    ├── logs
    │   └── winbrew.log
    └── cache
```

### Winget manifests

WinBrew now resolves manifests from Winget-compatible YAML files by default.

Default source settings:
- repository root: `https://raw.githubusercontent.com/microsoft/winget-pkgs/master`
- manifest format: `yaml`
- manifest kind: `installer`
- path template: `manifests/${partition}/${publisher}/${package}/${version}/${identifier}.${kind}.yaml`

Example:

```yaml
PackageIdentifier: Microsoft.WindowsTerminal
PackageVersion: 1.9.1942.0
Installers:
  - Architecture: x64
    InstallerType: msix
    InstallerUrl: https://github.com/microsoft/terminal/releases/download/v1.9.1942.0/Microsoft.WindowsTerminal_1.9.1942.0_8wekyb3d8bbwe.msixbundle
    InstallerSha256: 578D987D58B3CE5F6BF3316C6A5AECE8EB6B94DBCD1963413D81CB313D6C28D5
ManifestType: installer
ManifestVersion: 1.10.0
```

> [!NOTE]
> Only `portable` and `msi` installer kinds are supported for now. Any other installer type will fail during manifest validation.

## Usage

### Install package
```bash
brew install windows terminal
brew install Microsoft.WindowsTerminal --version 1.9.1942.0
```

If multiple packages match the query, WinBrew shows a numbered list and asks you to pick one.

### List installed packages
```bash
brew list
```

### Remove package
```bash
brew remove node         # interactive confirmation
brew remove node --yes   # skip confirmation
```

### Config
```bash
brew config list
brew config get core.log_level
brew config get core.file_log_level
brew config set core.log_level debug
brew config set core.file_log_level "debug,winbrew::core::network=trace"
```

Config is stored in `C:\winbrew\data\winbrew.toml` and it's like this:

```toml
[core]
log_level = "info"
file_log_level = "debug,winbrew::core::network=trace"
auto_update = true
confirm_remove = true
default_yes = false
color = true
download_timeout = 30
concurrent_downloads = 3
github_token = ""
proxy = ""

[paths]
root = "C:\\winbrew"
packages = "${root}\\packages"
data = "${root}\\data"
logs = "${root}\\data\\logs"
cache = "${root}\\data\\cache"

[sources]
primary = "winget"

[sources.winget]
url = "https://raw.githubusercontent.com/microsoft/winget-pkgs/master"
format = "yaml"
manifest_kind = "installer"
manifest_path_template = "manifests/${partition}/${publisher}/${package}/${version}/${identifier}.${kind}.yaml"
enabled = true
```

> [!NOTE]
> Logging is split across two settings:
> - core.log_level controls what appears in the terminal.
> - core.file_log_level controls the background log file and accepts full EnvFilter strings.

For a quieter log file:
```toml
file_log_level = "warn,winbrew::core::network=trace"
```

## License

`WinBrew` is dual-licensed under **MIT OR Apache-2.0**.

See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
