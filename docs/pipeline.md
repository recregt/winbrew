# WinBrew Catalog Pipeline

This page documents the current decision set for the Winget catalog pipeline.

The goal is a full offline catalog: Winget manifests are ingested into `catalog.db`, installer URLs and scenarios are materialized ahead of time, and runtime install/search paths never need to reach Winget or GitHub APIs.

## Decision Summary

- Winget `index.db` is a revision anchor, not the source of truth.
- Winget YAML manifests are the source of truth for installer URLs, hashes, switches, architecture, and scope.
- `catalog.db` is the canonical offline artifact that install, search, and update consume.
- R2 is the delivery plane. It stores versioned artifacts, but it is not the source of truth.
- `api.winbrew.dev` is the update selection plane. It decides whether the client should fetch a full snapshot or patch chain.
- `metadata.json` is the control-plane manifest used to choose the right update artifact.
- `metadata.json` is published last through a temp-key and copy-replace flow so clients never see a partial write.
- Full snapshots are the baseline transport format; package-level deltas are the first incremental optimization.
- Delta chains fall back to a full snapshot when there are more than 7 patches or when a single patch exceeds 40% of the full snapshot size.
- The catalog publish workflow materializes `release_lineage` and `patch_artifacts` into D1, then materializes update plan rows so the worker stays lookup-only at request time.
- `catalog_packages_fts` stays in place and should be preserved through incremental writes.

## Source Model

The Winget feed has three relevant layers:

1. `source.msix` provides the staged Winget index database.
2. `index.db` tells us which packages and versions exist.
3. Manifest records provide the installer detail that `catalog_installers` needs.

The pipeline should treat the first two as discovery and revision inputs, then enrich the data from manifests before writing the catalog.

In practice, that enrichment happens in the crawler: it merges the staged index rows with manifest YAML and emits one Winget JSONL stream for the parser.

## Ingest Strategy

The crawler should be manifest-aware.

That means:

- enumerate Winget packages from `index.db`
- fetch the manifest or API records for changed packages
- reconstruct installer rows with `url`, `hash`, `hash_algorithm`, `arch`, `kind`, `nested_kind`, and `scope` when available
- cache upstream artifacts so unchanged manifests are not fetched again

The first full crawl can be slow because it fan-outs across a large manifest set. Incremental crawls should only be fast when the crawler can prove that a package or manifest changed.

The catalog database schema version is currently `2`. Version `1` catalog files are intentionally invalid and must be rebuilt from the parser output.

Rules:

- keep `scope` optional for now
- store the catalog schema version in `schema_meta`
- treat the schema version as an explicit catalog contract, not an implied parser detail
- avoid deferring this decision into a later migration unless the table shape itself changes

## Crawler Backoff

Winget API calls should use exponential backoff from the beginning.

The crawler should retry transient failures such as throttling, temporary network errors, and 5xx responses. A simple policy is:

- start at 1 second
- double the delay on each retry
- stop after 5 attempts

That keeps the first full crawl from failing immediately when Winget starts rate limiting the deep manifest walk.

## Catalog Shape

The catalog should keep the current normalized shape, but Winget entries must be fully populated.

Important rules:

- keep `arch` as the canonical column name unless a later migration proves otherwise
- add `scope` as a first-class selector dimension if the upstream manifest exposes it
- do not hide scope inside `installer_switches`
- keep `hash` and `hash_algorithm` as the payload verification contract
- treat `platform`, `commands`, `protocols`, `file_extensions`, and `capabilities` as mergeable metadata, not installer identity
- deduplicate installers by the canonical identity columns only
- add a separate signature field only if the upstream feed exposes a distinct signer or signature hash

The parser should emit one installer row per installer variant, not collapse architecture-specific or scope-specific entries into a single record.

## Storage Strategy

The SQLite writer should move away from destructive rebuilds.

The desired behavior is:

- open or migrate the existing catalog
- upsert changed packages
- delete stale installers only for the package being rewritten
- preserve raw package JSON for traceability
- keep a sync-state record so later delivery can diff against a known upstream revision

This keeps the catalog stable enough for delta generation and update selection.

## Delivery Strategy

R2 should publish two classes of artifact:

- full snapshots, compressed with zstd
- package-level deltas under `patches/`

The publication contract should use `metadata.json` to describe the current artifact, the previous artifact, and any compatibility or lineage data needed by the client.

Raw SQLite page diffs or `sqldiff`-only delivery can be considered later, but the first incremental step should be package-granularity deltas.

## Metadata Write Order

R2 publication must expose a complete artifact set before the control-plane metadata points at it.

The sequence should be:

1. upload the new catalog snapshot or patch payload under a temporary key
2. upload `metadata.json.tmp`
3. copy or replace `metadata.json.tmp` to `metadata.json` as the final publish step
4. let the API and CLI read only the final `metadata.json`

That order prevents a partially uploaded bundle from being treated as a valid release.

## Update API Gateway

The CLI should not contain R2 object URLs or bucket-specific routing logic.

Instead, update selection should go through `https://api.winbrew.dev/v1/update`.

Request shape:

- the CLI sends the current installed catalog version, for example `current=v100`
- additional fields can be added later for channel, platform, or architecture without changing the selection model

Worker behavior:

1. read the current request version
2. look up the precomputed plan in D1
3. return `current` when the client is already on the latest catalog
4. return the full snapshot link when the change set is too large or the update chain must reset
5. return the matching `.sql.zst` patch links when the delta is small enough to apply incrementally

The worker should return CDN URLs, not raw R2 object URLs, and it should not traverse the release graph at request time.

Delta threshold rules:

- if the patch chain is longer than 7 patches, return the full snapshot
- if any single patch is larger than 40% of the full snapshot size, return the full snapshot
- otherwise return the ordered patch chain
- compute the threshold in the publish/materialization step, not at request time

Planned response shape:

```json
{
	"mode": "current",
	"current": "v100",
	"target": "v100",
	"patches": []
}
```

```json
{
	"mode": "full",
	"current": "v100",
	"target": "v101",
	"snapshot": "https://cdn.winbrew.dev/catalog/latest.db.zst",
	"patches": []
}
```

For a patchable update, `mode` should switch to `patch` and `patches` should contain the ordered `.sql.zst` links to apply.

## Bootstrap and Update Flow

The CLI should bootstrap and update from the local catalog model, not from Winget runtime lookups.

The update flow should:

1. read the local installed catalog version
2. call `https://api.winbrew.dev/v1/update`
3. decide whether to download a full snapshot, patch chain, or no-op current response from the API response
4. if the patch chain fails, re-query the API for a full snapshot plan instead of using a hardcoded bucket URL
5. show progress while downloading
6. verify hashes before applying anything
7. swap the local DB atomically
8. run integrity checks and roll back on failure

The existing `crates/app/src/operations/update/mod.rs` flow is the natural home for this behavior, but the update selector should live behind the API instead of being hardcoded into the CLI.

## Implementation Order

1. Lock the schema.
2. Make metadata writes atomic.
3. Add crawler backoff.
4. Add delta threshold logic.
5. Run the first full crawl.
6. Validate incremental updates with tests.

## GitHub Actions Model

Scheduled catalog builds should split work by day:

- Sunday: full build, full crawl, parser rebuild, full snapshot publish
- weekdays: incremental crawl, incremental parser write, delta publish
- After a successful publish, the workflow materializes `release_lineage`, `patch_artifacts`, and `update_plans` into the production D1 database so the update worker stays lookup-only at request time.

The workflow should stay under GitHub Actions runtime limits by relying on cache reuse, concurrency control, and sharding if the first crawl regularly approaches the job limit.

Expected behavior:

- a first deep crawl can take hours at Winget scale
- repeat crawls should only become minutes long if manifest change tracking is effective
- R2 upload should happen through the publisher, with credentials injected by secrets

## Safety Rules

- always back up the local DB before applying a patch
- run `PRAGMA integrity_check` after update
- fall back to the latest full snapshot by re-querying the update API if a delta chain fails verification or application
- keep install and search local-first and catalog-first

## Files That Matter

- [infra/crawler/README.md](../infra/crawler/README.md)
- [infra/parser/README.md](../infra/parser/README.md)
- [infra/publisher/README.md](../infra/publisher/README.md)
- [infra/crawler/pkg/sources/winget/winget.go](../infra/crawler/pkg/sources/winget/winget.go)
- [infra/crawler/pkg/sources/scoop/scoop.go](../infra/crawler/pkg/sources/scoop/scoop.go)
- [infra/parser/src/winget.rs](../infra/parser/src/winget.rs)
- [infra/parser/src/sqlite.rs](../infra/parser/src/sqlite.rs)
- [infra/parser/src/pipeline.rs](../infra/parser/src/pipeline.rs)
- [infra/parser/schema/catalog.sql](../infra/parser/schema/catalog.sql)
- [crates/database/src/catalog.rs](../crates/database/src/catalog.rs)
- [infra/publisher/internal/publisher/metadata.go](../infra/publisher/internal/publisher/metadata.go)
- [infra/publisher/internal/publisher/publisher.go](../infra/publisher/internal/publisher/publisher.go)
- [crates/app/src/operations/update/mod.rs](../crates/app/src/operations/update/mod.rs)
- [.github/workflows/catalog.yml](../.github/workflows/catalog.yml)

## Open Questions

- How much manifest cache should live in GitHub Actions cache versus on the crawler host cache?
- When does it become worth introducing a hard size ceiling or shard boundary for the first full crawl?
