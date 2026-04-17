# WinBrew Build Guide

This page contains the local build, test, and run path that used to live in the README.

## Prerequisites

- Windows 10 or Windows 11
- Git
- Rust toolchain from [rust-toolchain.toml](../rust-toolchain.toml)
- Go toolchain for repo-local helper tasks
- [go-task](https://taskfile.dev/)
- PowerShell 7 or later

## 1. Clone the repository

```powershell
git clone https://github.com/recregt/winbrew.git
Set-Location winbrew
```

## 2. Install local tooling

These tools are used by the repository tasks and local checks:

```powershell
task tools:install-lefthook
task tools:install-nextest
task tools:install-golangci-lint
lefthook install
```

## 3. Verify the Rust toolchain

The repository pins the Rust channel in [rust-toolchain.toml](../rust-toolchain.toml), so the local toolchain should match that file before you build.

```powershell
rustup show
cargo --version
task check
```

## 4. Run the Rust checks

```powershell
task ci:rust
task test:nextest
```

`task ci:rust` runs formatting, clippy, docs, and the CLI test suite. `task test:nextest` is useful when you want the Rust tests without the extra CI wrapping.

## 5. Build the CLI binary

```powershell
cargo build --locked -p winbrew-bin --bin winbrew
```

That produces the local `winbrew` executable under `target\debug`. If you want an optimized local build, use:

```powershell
task dev:run-release -- version
```

The task name says `release`, but it only means a local `--release` build. It is not a published release artifact.

## 6. Run locally

The repository provides a dev root so local runs do not pollute your profile:

```powershell
task dev:run -- version
task dev:run -- doctor
task dev:run -- list
```

You can pass any WinBrew arguments after `--`, for example `task dev:run -- install firefox` or `task dev:run -- search git`.

## 7. Reset the dev root

```powershell
task dev:clean
```

This removes the repo-local `target\winbrew-dev` tree.

## Related Docs

- [README](../README.md) for the user-facing overview and FAQ.
- [docs/index.md](index.md) for the documentation map.
- [Contributing](../CONTRIBUTING.md) for contributor workflow and validation commands.
