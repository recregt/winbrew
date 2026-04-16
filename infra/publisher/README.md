# WinBrew Catalog Publisher

The publisher is the deployment stage for the catalog bundle. It validates the local `catalog.db` against its metadata and uploads both artifacts to R2.

## What it does

- Loads publish settings from environment variables.
- Resolves the input database and metadata paths.
- Verifies that the local metadata hash matches the input database.
- Reads the remote metadata object from the bucket.
- Skips publishing when the remote catalog already matches the local hash.
- Uploads the database to a temporary key, then publishes the metadata and final object when a new bundle is available.
- Writes the updated local metadata back to disk after a successful upload.
- Optionally emits `update_plans.sql` for the production D1 database after a successful publish.
- Can emit a patch plan for the previous hash when a normalized D1 patch-chain manifest is available.

## Inputs

### CLI flags

- `--input`: path to the catalog database. Defaults to `WINBREW_DB_PATH` if set.
- `--metadata`: path to the local metadata file. Defaults to `metadata.json` beside the input database.
- `--key`: object key for the database in the bucket. Defaults to `catalog.db`.
- `--update-plans`: optional path for the D1 materialization SQL file. The file is only written after a successful publish.
- `--patch-chain`: optional normalized JSON manifest of D1 patch artifacts. When present, the publisher can emit a patch plan instead of a fallback full snapshot for the previous hash.

### Environment variables

- `R2_ENDPOINT`: R2 endpoint URL or host.
- `R2_BUCKET_NAME`: destination bucket name.
- `R2_ACCESS_KEY_ID` / `AWS_ACCESS_KEY_ID`: access key.
- `R2_SECRET_ACCESS_KEY` / `AWS_SECRET_ACCESS_KEY`: secret key.
- `R2_REGION`: optional bucket region, defaults to `auto`.
- `CATALOG_PUBLIC_BASE_URL`: public base URL used to build update-plan snapshot URLs. Defaults to `https://cdn.winbrew.dev`.
- The patch-chain manifest is expected to contain D1 query rows with `depth`, `file_path`, `size_bytes`, and `reached_previous` fields.

## Outputs

- Remote object `catalog.db`: the SQLite catalog database, published from a temporary staging key.
- Remote object `metadata.json`: the metadata sidecar associated with that database.
- Local metadata file: updated with the previous remote hash after a successful publish.
- Optional `update_plans.sql`: SQL statements that clear and repopulate the current production D1 update rows.
- Optional patch-chain manifest: normalized D1 query output used to choose a patch plan when the incremental chain is short enough.

## Publish contract

The publisher expects a bundle produced by the parser:

- `catalog.db` must be the final database file.
- `metadata.json` must describe that exact database hash.
- The metadata schema version must match the current parser format.

If the remote metadata already points at the same hash, the publisher exits successfully without re-uploading the bundle.

## Errors and exit codes

- `0`: success
- `1`: publish failure, including missing R2 settings, hash mismatch, remote read failure, or upload failure

## Example

```powershell
Set-Location infra/publisher
go run ./cmd/publisher --input ..\staging\catalog.db --metadata ..\staging\metadata.json
```

Typical usage is to publish the bundle produced by the parser:

```powershell
Set-Location infra/publisher
$env:R2_ENDPOINT = "https://example.r2.cloudflarestorage.com"
$env:R2_BUCKET_NAME = "winbrew"
$env:R2_ACCESS_KEY_ID = "..."
$env:R2_SECRET_ACCESS_KEY = "..."
go run ./cmd/publisher --input ..\staging\catalog.db --metadata ..\staging\metadata.json
```
