# WinBrew Crawler

The crawler is the ingestion stage for the catalog pipeline. It fetches upstream package sources, normalizes them into a JSONL stream, and writes the Winget package feed as a JSONL artifact for the parser.

## What it does

- Loads and validates `config.yaml`.
- Builds source clients for the configured sources (`scoop`, `winget`).
- Streams Scoop packages as JSONL to `stdout`.
- Writes the Winget JSONL artifact to `--winget-out`.
- Keeps logs on `stderr` so the JSONL stream stays clean.
- Uses retry, timeout, and cache settings from the config file.

## Inputs

- `--config`: path to the crawler config file. Defaults to `config.yaml`.
- `--winget-out`: destination path for the staged Winget JSONL artifact.

The config controls:

- `sources`: enabled sources, currently `scoop` and `winget`.
- `logLevel`: `debug`, `info`, `warn`, or `error`.
- `timeout.fetch`: HTTP fetch timeout for source requests.
- `retry.max` and `retry.backoff`: retry policy for transient source failures.

## Outputs

- `stdout`: Scoop package JSONL, one envelope per line.
- `--winget-out`: staged Winget JSONL file.
- `stderr`: structured logs and diagnostics.

## Runtime model

The crawler does not build the catalog database itself. Its job is to produce two artifacts that the parser can consume:

1. a JSONL stream of Scoop packages
2. a Winget JSONL file on disk

The parser merges those inputs into the final catalog bundle.

## Errors and exit codes

- `0`: success
- `1`: runtime failure
- `2`: invalid arguments or missing required config
- `130`: cancelled by signal

## Example

```powershell
Set-Location infra/crawler
go run ./cmd/crawler --config config.yaml --winget-out ..\staging\winget_source.jsonl
```

To build the full bundle, pipe the crawler into the parser starting from the repository root:

```powershell
Set-Location infra/crawler; go run ./cmd/crawler --winget-out ..\staging\winget_source.jsonl | cargo run --manifest-path ..\..\Cargo.toml -p winbrew-infra-parser -- --winget-jsonl ..\staging\winget_source.jsonl --out ..\staging\catalog.db --metadata ..\staging\metadata.json
```
