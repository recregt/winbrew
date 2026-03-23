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
├── bin
├── packages
└── data
	├── winbrew.toml
	├── db
	│   └── winbrew.db
	├── logs
	│   └── winbrew.log
	└── cache
```

## Usage

### List installed packages
```bash
brew list
```

### Install package
```bash
brew install node
brew install ripgrep latest
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
brew config set core.log_level debug
```

Config is stored in `C:\winbrew\data\winbrew.toml` and uses typed sections:
- `core`
- `paths`
- `sources`

## License

`winbrew` is dual-licensed under **MIT OR Apache-2.0**.

See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
