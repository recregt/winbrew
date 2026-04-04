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

Copy and paste this command to Powershell (Admin):

```powershell
PowerShell -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/recregt/winbrew/main/scripts/install.ps1 | iex"
```

## Development

Install the repository git hooks:

```powershell
task hooks:install
```

Run the same checks locally without going through Git:

```powershell
task hooks:pre-commit
task hooks:pre-push
```

### Installed layout

By default, `winbrew` layout looks like this:

```text
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
winbrew list
```

### Show runtime info
```bash
winbrew info
```

### Check health
```bash
winbrew doctor
```

### Install package
```bash
winbrew install node
```

### Remove package
```bash
winbrew remove node
winbrew remove node --yes
```

### Config
```bash
winbrew config list
winbrew config get core.log_level
winbrew config set core.log_level debug
winbrew config set core.file_log_level "debug,winbrew::core=trace"
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
