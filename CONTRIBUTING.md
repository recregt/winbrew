# Contributing

**`WinBrew`** uses **[go-task](https://taskfile.dev/)** and **[Lefthook](https://lefthook.dev/)** to manage the development workflow.

For implementation details of the catalog pipeline stages, see **[crawler](infra/crawler/README.md)**, **[parser](infra/parser/README.md)**, and **[publisher](infra/publisher/README.md)**.

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
| `task ci:verify` | Run the full CI task set locally, including bundle validation |
| `task ci:go:crawler` | Run crawler Go checks |
| `task ci:go:publisher` | Run publisher Go checks for the catalog bundle flow |
| `task ci:rust` | Run Rust checks for the catalog parser bundle and CLI |
| `task ci:smoke` | Build and smoke-test the CLI |
| `task dev:run -- <args>` | Run locally without polluting your profile |
| `task dev:run-release -- <args>` | Run in release mode |
| `task dev:clean` | Clean the dev root |

`task dev:run` and `task dev:run-release` use `target\winbrew-dev` via `WINBREW_PATHS_ROOT`, so config, logs, and databases stay inside the repo.

You can pass any WinBrew arguments after `--`, for example `task dev:run -- doctor` or `task dev:run-release -- install firefox`.

## Catalog Bundle Pipeline

For a local end-to-end smoke test of the catalog bundle flow, run the crawler and parser as a pipe from the repository root:

```powershell
Set-Location infra/crawler; go run ./cmd/crawler --winget-out ..\staging\winget_source.db | cargo run --manifest-path ..\..\Cargo.toml -p winbrew-infra-parser -- --winget-db ..\staging\winget_source.db --out ..\staging\catalog.db --metadata ..\staging\metadata.json
```

That command streams Scoop JSONL into the parser, stages the Winget database on disk, and writes the catalog bundle (`catalog.db` + `metadata.json`) into the shared staging folder.

The publisher then uploads the same bundle using the required R2 environment variables.

The GitHub Actions workflow in [.github/workflows/catalog.yml](.github/workflows/catalog.yml) runs the same flow on Ubuntu on manual dispatch and every 6 hours using the repository secrets `R2_ACCESS_KEY_ID`, `R2_SECRET_ACCESS_KEY`, and `CLOUDFLARE_ACCOUNT_ID`; `R2_BUCKET_NAME` can be overridden with a repo variable and defaults to `winbrew`.

### Exit Codes

| Stage | Exit Code | Meaning | Typical Cause |
| --- | --- | --- | --- |
| Crawler | 0 | Success | Both sources completed and the staged Winget database was written |
| Crawler | 1 | Failure | Config load failure, source fetch failure, retry exhaustion, or cancellation surfaced as an error |
| Catalog Parser | 0 | Success | JSONL and Winget input were merged, and the catalog bundle (`catalog.db` + `metadata.json`) was written |
| Catalog Parser | 1 | Pipeline failure | Winget read failure, SQLite write failure, hash failure, or any other runtime error |
| Catalog Parser | 2 | Usage error | Missing required arguments or an invalid CLI flag |
| Catalog Publisher | 0 | Success | Local hash matched metadata, or the catalog bundle was uploaded successfully |
| Catalog Publisher | 1 | Failure | Missing R2 settings, hash mismatch, remote metadata read failure, or upload failure |
