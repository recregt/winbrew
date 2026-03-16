# winbrew (`wb`)

[![CI](https://github.com/recregt/winbrew/actions/workflows/ci.yml/badge.svg)](https://github.com/recregt/winbrew/actions)
[![Release](https://github.com/recregt/winbrew/actions/workflows/release.yml/badge.svg)](https://github.com/recregt/winbrew/actions)
[![Version](https://img.shields.io/github/v/release/recregt/winbrew?color=blue&logo=github&label=Version)](https://github.com/recregt/winbrew/releases/latest)

A modern package manager for Windows that installs, tracks, and cleanly removes software.

> ⚠️ Early development. Install and track features are not yet implemented.

## Requirements

- Windows 10 or later
- Run as **Administrator** (required for registry access)

## Installation

### From source
```bash
cargo install --path .
```

### From release

Download the latest `winbrew-vX.X.X-windows-x86_64.zip` from [Releases](https://github.com/recregt/winbrew/releases), extract it, and place `wb.exe` somewhere in your `PATH`.

## Usage

### List installed applications
```bash
wb list
wb list steam        # filter by name
```

### Scan for leftovers

Scans registry and common directories without making any changes.
```bash
wb scan steam
```

### Clean leftovers
```bash
wb clean steam             # interactive confirmation
wb clean steam --yes       # skip confirmation
wb clean steam --dry-run   # preview without deleting
```

## How it works

`wb scan` and `wb clean` search the following locations:

**Registry**
- `HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`
- `HKLM\SOFTWARE\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall`
- `HKCU\SOFTWARE\Microsoft\Windows\CurrentVersion\Uninstall`

**Directories**
- `%PROGRAMFILES%`
- `%PROGRAMFILES(X86)%`
- `%APPDATA%`
- `%LOCALAPPDATA%`
- `%LOCALAPPDATA%\..\LocalLow`
