# WinBrew

[![CI](https://github.com/recregt/winbrew/actions/workflows/main.yml/badge.svg)](https://github.com/recregt/winbrew/actions)

A modern package manager for Windows that installs, tracks, and cleanly removes software.

> [!IMPORTANT]
> This project is currently in active development. Public releases are not being
> published yet, and there is no supported end-user installer flow.

For the architecture and documentation map, start with [docs/index.md](docs/index.md). For engine-specific behavior and ownership, see [docs/engines.md](docs/engines.md).

## Build From Source

WinBrew is intended to be built from source while development is ongoing. If
you want to try it locally, use the steps below.

### Prerequisites

- Windows 10 or Windows 11
- Git
- Rust toolchain from [rust-toolchain.toml](rust-toolchain.toml)
- Go toolchain for repo-local helper tasks
- [go-task](https://taskfile.dev/)
- PowerShell 7 or later

### 1. Clone the repository

```powershell
git clone https://github.com/recregt/winbrew.git
Set-Location winbrew
```

### 2. Install local tooling

These tools are used by the repository tasks and local checks:

```powershell
task tools:install-lefthook
task tools:install-nextest
task tools:install-golangci-lint
lefthook install
```

### 3. Verify the Rust toolchain

The repository pins the Rust channel in `rust-toolchain.toml`, so the local
toolchain should match that file before you build.

```powershell
rustup show
cargo --version
task check
```

### 4. Run the Rust checks

```powershell
task ci:rust
task test:nextest
```

`task ci:rust` runs formatting, clippy, docs, and the CLI test suite. `task
test:nextest` is useful when you want the Rust tests without the extra CI
wrapping.

### 5. Build the CLI binary

```powershell
cargo build --locked -p winbrew-bin --bin winbrew
```

That produces the local `winbrew` executable under `target\debug`. If you want
an optimized local build, use:

```powershell
task dev:run-release -- version
```

The task name says `release`, but it only means a local `--release` build. It is
not a published release artifact.

### 6. Run locally

The repository provides a dev root so local runs do not pollute your profile:

```powershell
task dev:run -- version
task dev:run -- doctor
task dev:run -- list
```

You can pass any WinBrew arguments after `--`, for example `task dev:run --
install firefox` or `task dev:run -- search git`.

### 7. Reset the dev root

```powershell
task dev:clean
```

This removes the repo-local `target\winbrew-dev` tree.

### Basic Usage

The commands below are available after you build and run WinBrew locally.

| Command | Description |
| :--- | :--- |
| `winbrew config` | Inspect or update runtime configuration |
| `winbrew doctor` | Check system health and configuration |
| `winbrew info <pkg>` | Show package details |
| `winbrew install <pkg>` | Install a package |
| `winbrew list` | List installed packages |
| `winbrew remove <pkg>` | Remove a package and its leftovers |
| `winbrew search <query>` | Search for a package |
| `winbrew update` | Refresh the catalog data |
| `winbrew version` | Print the binary version |
| `winbrew repair` | Repair installed state and recovery trails |

### File Layout

By default, WinBrew isolates everything within the current user's local app data directory:

`%LOCALAPPDATA%\winbrew`

```text
%LOCALAPPDATA%\winbrew
├── packages    # Installed applications
└── data
    ├── db      # SQLite metadata (winbrew.db, catalog.db)
    ├── pkgdb   # Per-package recovery journals
    ├── logs    # Rolling execution logs
    ├── cache   # Downloaded installers/temporary files
    └── winbrew.toml  # Persisted runtime configuration
```

Package-scoped evidence, when emitted, is documented in
[docs/managed-paths-policy.md](docs/managed-paths-policy.md) and uses
`data/logs/packages/<package-key>/`.

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

Development setup and contributor tasks are documented in
[CONTRIBUTING.md](CONTRIBUTING.md).

The full docs map lives in [docs/index.md](docs/index.md), and engine-specific guidance lives in [docs/engines.md](docs/engines.md).

If you want the shortest path to a clean local environment, use the following
task sequence after cloning:

```powershell
task tools:install-lefthook
task tools:install-nextest
task tools:install-golangci-lint
lefthook install
task check
task ci:rust
task ci:smoke
```

Detailed technical docs for the catalog pipeline live in:

- [infra/crawler/README.md](infra/crawler/README.md)
- [infra/parser/README.md](infra/parser/README.md)
- [infra/publisher/README.md](infra/publisher/README.md)

## License

WinBrew is dual-licensed under **[MIT](LICENSE-MIT)** and **[Apache-2.0](LICENSE-APACHE)**.
