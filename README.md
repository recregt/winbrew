# winbrew (`wb`)

[![CI](https://github.com/recregt/winbrew/actions/workflows/ci.yml/badge.svg)](https://github.com/recregt/winbrew/actions)
[![Release](https://github.com/recregt/winbrew/actions/workflows/release.yml/badge.svg)](https://github.com/recregt/winbrew/actions)
[![Version](https://img.shields.io/github/v/release/recregt/winbrew?include_prereleases&color=blue&logo=github&label=Version)](https://github.com/recregt/winbrew/releases/latest)

A modern package manager for Windows that installs, tracks, and cleanly removes software.

> [!IMPORTANT]
> Early development. Core install/list/remove+ flows are available and still evolving.

## Requirements

- Windows 10 or later
- Internet access (for package manifests and artifacts)

## Installation

### From source
```bash
cargo install --path .
```

### From release

Download the latest `winbrew-vX.X.X-windows-x86_64.zip` from [Releases](https://github.com/recregt/winbrew/releases), extract it, and place `wb.exe` somewhere in your `PATH`.

## Usage

### List installed packages
```bash
wb list
```

### Install package
```bash
wb install node
wb install ripgrep latest
```

### Remove package
```bash
wb remove node         # interactive confirmation
wb remove node --yes   # skip confirmation
```

## How it works

`wb install` does the following:

- Fetches package manifest from the configured package repository
- Downloads and verifies artifact checksum
- Extracts package under `%WINBREW_ROOT%\packages\<name>`
- Creates shims under `%WINBREW_ROOT%\bin`
- Persists metadata into `%WINBREW_ROOT%\data\winbrew.db`

`wb remove` removes shims and package directory, then deletes package metadata.

Default root is `C:\winbrew`, override with `WINBREW_ROOT`.

### Package repository

Current manifest source:
- `https://raw.githubusercontent.com/recregt/winbrew-pkgs/main/<name>/<version>.toml`

### Legacy cleanup mode

Currently, registry scan/cleanup command group (`scan`/`clean`) is removed.

## License

`winbrew` is dual-licensed under **MIT OR Apache-2.0**.

See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE) for details.
