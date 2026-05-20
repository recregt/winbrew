# WinBrew Build Guide

This guide outlines the steps to set up the development environment, build, test, and run WinBrew locally from source.

## Prerequisites

Ensure you have the following installed on your system before proceeding:

- Windows 10 or Windows 11
- Git
- Rust toolchain (version specified in [`rust-toolchain.toml`](../rust-toolchain.toml))
- Go toolchain (required for repo-local helper tasks)
- [go-task](https://taskfile.dev/)
- PowerShell 7 or later

## Automated Installation

This script builds WinBrew from source and installs the binary to `C:\winbrew`. 
It does **not** set up a development environment — the source checkout and build 
artifacts are removed automatically after installation.

Run PowerShell as an administrator and execute the following command:

```powershell
irm https://raw.githubusercontent.com/recregt/winbrew/main/scripts/install.ps1 | iex
```

Alternatively, if you have already downloaded the script file locally:

```powershell
.\install.ps1
```

## Manual Build Process

If you prefer to build the project manually or need to set up a development environment, follow the steps below.

### 1. Clone the Repository

```powershell
git clone https://github.com/recregt/winbrew.git
Set-Location winbrew
```

### 2. Install Local Tooling

These tools are required for repository tasks, linting, and local checks:

```powershell
task tools:install-lefthook
task tools:install-nextest
task tools:install-golangci-lint
lefthook install
```

### 3. Verify the Rust Toolchain

The repository pins the Rust channel in [`rust-toolchain.toml`](../rust-toolchain.toml). Ensure your local toolchain matches the required version before building:

```powershell
rustup show
cargo --version
task check
```

### 4. Run Checks and Tests

Run the following tasks to ensure everything is functioning correctly:

```powershell
task ci:rust
task test:nextest
```

* `task ci:rust`: Runs formatting, Clippy, documentation generation, and the CLI test suite.
* `task test:nextest`: Runs only the Rust tests without the extra CI wrapping.

### 5. Build the CLI Binary

To build the executable for debugging and development:

```powershell
cargo build --locked -p winbrew-bin --bin winbrew
```

This compiles the local `winbrew` executable into the `target\debug` directory.

If you want to test an optimized local build, run:

```powershell
task dev:run-release -- version
```

> **Note:** The `run-release` task compiles a local `--release` build; it does not produce a published release artifact.

### 6. Run Locally

The repository provides an isolated development root so local execution does not pollute your main profile:

```powershell
task dev:run -- version
task dev:run -- doctor
task dev:run -- list
```

You can pass any WinBrew arguments after the `--` separator. For example:

* `task dev:run -- install firefox`
* `task dev:run -- search git`

### 7. Clean Up the Dev Environment

To reset your development root and clean up local artifacts:

```powershell
task dev:clean
```

This command removes the repository-local `target\winbrew-dev` tree.