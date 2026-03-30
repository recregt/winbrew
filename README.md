# WinBrew

[![CI](https://github.com/recregt/winbrew/actions/workflows/ci.yml/badge.svg)](https://github.com/recregt/winbrew/actions)
[![Release](https://github.com/recregt/winbrew/actions/workflows/release.yml/badge.svg)](https://github.com/recregt/winbrew/actions)
[![Version](https://img.shields.io/github/v/release/recregt/winbrew?include_prereleases&color=blue&logo=github&label=Version)](https://github.com/recregt/winbrew/releases/latest)

A modern package manager for Windows that tracks and cleanly removes software.

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

## Usage

### List installed packages
```bash
brew list
```

### Show runtime info
```bash
brew info
```

### Check health
```bash
brew doctor
```

### Remove package
```bash
brew remove node
brew remove node --yes
```

### Config
```bash
brew config list
brew config get core.log_level
brew config set core.log_level debug
brew config set core.file_log_level "debug,winbrew::core=trace"
```

Config is stored in `C:\winbrew\data\winbrew.toml` and it's like this:

```toml
[core]
log_level = "info"
file_log_level = "debug,winbrew::core=trace"
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
```

> [!TIP]
> Logging is split across two settings:
> - core.log_level controls what appears in the terminal.
> - core.file_log_level controls the background log file and accepts full EnvFilter strings.

For a quieter log file:
```toml
file_log_level = "warn,winbrew::core=trace"
```

## License

`WinBrew` is dual-licensed under **MIT OR Apache-2.0**.

See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
