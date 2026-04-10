# WinBrew Catalog Parser

The parser is the transformation stage of the catalog pipeline. It consumes the crawler output plus the staged Winget database, writes the final SQLite catalog, and emits the catalog metadata sidecar.

## What it does

- Reads Scoop package envelopes from `stdin`.
- Loads the staged Winget database from `--winget-db`.
- Normalizes both sources into the shared catalog schema.
- Accepts common Winget version variants, normalizes them into semver-compatible catalog versions, and only skips entries that are still truly unrecoverable.
- Writes the catalog database to `--out`.
- Writes `metadata.json` next to the output database unless `--metadata` overrides it.
- Hashes the final database and stores the digest in the metadata bundle.

## Inputs

- `--winget-db`: staged Winget database produced by the crawler.
- `--out`: destination path for the SQLite catalog database.
- `--metadata`: optional path for the metadata sidecar. Defaults to `metadata.json` beside `--out`.
- `stdin`: Scoop JSONL stream from the crawler.

## Outputs

- `catalog.db`: SQLite catalog with `catalog_packages` and `catalog_installers` tables.
- `metadata.json`: JSON metadata bundle containing:
  - `schema_version`
  - `generated_at_unix`
  - `current_hash`
  - `previous_hash`
  - `package_count`
  - `source_counts`

## Pipeline contract

The parser is the point where the bundle becomes publishable.

1. It streams Scoop packages from `stdin`.
2. It reads Winget packages from the staged database.
3. It writes all normalized packages into SQLite.
4. It hashes the final database file.
5. It serializes catalog metadata with that hash.

The publisher consumes the database and metadata files produced here.

## Errors and exit codes

- `0`: success
- `1`: pipeline failure
- `2`: invalid arguments or missing required flags

## Example

```powershell
Set-Location infra/parser
cargo run --locked -- --winget-db ..\staging\winget_source.db --out ..\staging\catalog.db --metadata ..\staging\metadata.json
```

Typical end-to-end usage pipes the crawler into the parser:

```powershell
Set-Location infra/crawler; go run ./cmd/crawler --winget-out ..\staging\winget_source.db | cargo run --manifest-path ..\..\Cargo.toml -p winbrew-infra-parser -- --winget-db ..\staging\winget_source.db --out ..\staging\catalog.db --metadata ..\staging\metadata.json
```
