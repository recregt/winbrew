# WinBrew

[![CI](https://github.com/recregt/winbrew/actions/workflows/main.yml/badge.svg)](https://github.com/recregt/winbrew/actions)
[![Release](https://github.com/recregt/winbrew/actions/workflows/release.yml/badge.svg)](https://github.com/recregt/winbrew/actions)
[![Version](https://img.shields.io/github/v/release/recregt/winbrew?include_prereleases&color=blue&logo=github&label=Version)](https://github.com/recregt/winbrew/releases/latest)

A modern package manager for Windows that installs, tracks, and cleanly removes software.

> [!IMPORTANT]
> This project is currently in the **early stages** of development. Use with caution.

## Quick Start

### Installation

Run the following command in a **PowerShell (Admin)**:

```powershell
PowerShell -ExecutionPolicy Bypass -Command "irm https://raw.githubusercontent.com/recregt/winbrew/main/scripts/install.ps1 | iex"
```

### Basic Usage

| Command | Description |
| :--- | :--- |
| `winbrew search <query>` | Search for a package |
| `winbrew install <pkg>` | Install a new package |
| `winbrew remove <pkg>` | Remove a package and its leftovers |
| `winbrew list` | List all installed packages |
| `winbrew doctor` | Check system health and configuration |

### File Layout

By default, WinBrew isolates everything within the current user's local app data directory:

`%LOCALAPPDATA%\winbrew`

```text
%LOCALAPPDATA%\winbrew
├── packages    # Installed applications
└── data
    ├── db      # SQLite metadata (winbrew.db)
    ├── logs    # Rolling execution logs
    └── cache   # Downloaded installers/temporary files
```

## Configuration

Settings are stored in `%LOCALAPPDATA%\winbrew\data\winbrew.toml` by default.

Set `WINBREW_PATHS_ROOT` or `[paths].root` to use a different install root.

```toml
[core]
log_level = "info"
auto_update = true
confirm_remove = true

[paths]
root = "C:\\Users\\<you>\\AppData\\Local\\winbrew"
```

*Note: You can override any setting using environment variables with the `WINBREW_` prefix (e.g., `WINBREW_CORE_LOG_LEVEL=debug`).*

## Development

Development setup and contributor tasks are documented in [CONTRIBUTING.md](CONTRIBUTING.md).

## License

WinBrew is dual-licensed under **[MIT](LICENSE-MIT)** and **[Apache-2.0](LICENSE-APACHE)**.
