# Contributing

`WinBrew` uses **[go-task](https://taskfile.dev/)** and **[Lefthook](https://lefthook.dev/)** to manage the development workflow.

## Setup

```powershell
task tools:install-lefthook
task tools:install-nextest
task tools:install-golangci-lint
lefthook install
```

## Common Tasks

| Command | Description |
| :--- | :--- |
| `task test` | Run Rust tests |
| `task test:nextest` | Run Rust tests with nextest |
| `task ci:verify` | Run the full CI task set locally |
| `task ci:go:crawler` | Run crawler Go checks |
| `task ci:go:publisher` | Run publisher Go checks |
| `task ci:rust` | Run Rust checks |
| `task ci:smoke` | Build and smoke-test the CLI |
| `task dev:run -- <args>` | Run locally without polluting your profile |
| `task dev:run-release -- <args>` | Run in release mode |
| `task dev:clean` | Clean the dev root |

`task dev:run` and `task dev:run-release` use `target\winbrew-dev` via `WINBREW_PATHS_ROOT`, so config, logs, and databases stay inside the repo.

You can pass any WinBrew arguments after `--`, for example `task dev:run -- doctor` or `task dev:run-release -- install firefox`.

## Crawler Pipeline

For a local end-to-end smoke test of the new crawler flow, run the crawler and parser as a pipe from the repository root:

```powershell
Set-Location infra/crawler; go run ./cmd/crawler --winget-out ..\staging\winget_source.db | cargo run --manifest-path ..\..\Cargo.toml -p winbrew-infra-parser -- --winget-db ..\staging\winget_source.db --out ..\staging\catalog.db --metadata ..\staging\metadata.json
```

That command streams Scoop JSONL into the parser, stages the Winget database on disk, and writes catalog.db plus metadata.json into the shared staging folder.

The publisher is then run separately against the same artifacts with the required R2 environment variables.

### Exit Codes

| Stage | Exit Code | Meaning | Typical Cause |
| --- | --- | --- | --- |
| Crawler | 0 | Success | Both sources completed and the staged Winget database was written |
| Crawler | 1 | Failure | Config load failure, source fetch failure, retry exhaustion, or cancellation surfaced as an error |
| Parser | 0 | Success | JSONL and Winget input were merged, catalog.db was written, and metadata.json was emitted |
| Parser | 1 | Pipeline failure | Winget read failure, SQLite write failure, hash failure, or any other runtime error |
| Parser | 2 | Usage error | Missing required arguments or an invalid CLI flag |
| Publisher | 0 | Success | Local hash matched metadata, or the catalog and metadata were uploaded successfully |
| Publisher | 1 | Failure | Missing R2 settings, hash mismatch, remote metadata read failure, or upload failure |
