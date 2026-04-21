# WinBrew

![Windows](https://img.shields.io/badge/Windows-0078D6?style=flat&logo=windows&logoColor=white)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE-MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE-APACHE)

WinBrew is a catalog-first Windows package manager. It normalizes Winget and Scoop into a single SQLite catalog, publishes a fresh snapshot every day through GitHub Actions, and serves the catalog from cdn.winbrew.dev.

> [!IMPORTANT]
> **CLI is under active development and not yet released.**
> The catalog is available now. You can query it directly from SQLite or wait for the CLI to ship.

## Status

- Catalog snapshots are published daily and stored as both compressed and uncompressed SQLite artifacts.
- The refresh path is API-driven: the CLI asks the update API for a current, patch, or full plan before downloading catalog data.
- Repair replays committed package journals under `data/pkgdb/<package-key>/journal.jsonl` to rebuild package state after interrupted installs or disk drift.

---

## Using the Catalog Today

The catalog is a self-contained SQLite database. No CLI required.

```sh
# Download the latest catalog
curl -O https://cdn.winbrew.dev/catalog.db.zst

# Or grab the uncompressed version
curl -O https://cdn.winbrew.dev/catalog.db
```

```sh
# Search by name
sqlite3 catalog.db "SELECT id, name, version, source FROM catalog_packages WHERE name LIKE '%git%' LIMIT 10;"

# List all Scoop packages with a known binary
sqlite3 catalog.db "SELECT id, name, bin FROM catalog_packages WHERE source = 'scoop' AND bin IS NOT NULL LIMIT 10;"

# Full-text search
sqlite3 catalog.db "SELECT id, name FROM catalog_packages_fts WHERE catalog_packages_fts MATCH 'terminal' LIMIT 10;"
```

Older snapshots are published as dated nightly archives on [GitHub Releases](https://github.com/recregt/winbrew/releases).

## Catalog Schema

The SQLite catalog keeps the normalized data in a small set of tables:

| Table | Purpose |
| --- | --- |
| `catalog_packages` | Normalized package metadata from all sources. |
| `catalog_installers` | Per-installer rows with URL, hash, type, architecture, and scope. |
| `catalog_packages_raw` | Original upstream manifest payload. |
| `catalog_packages_fts` | Full-text search index over name, description, moniker, and tags. |

Package identity follows the pattern `<source>/<id>` — for example `winget/Git.Git` or `scoop/extras/alacritty`.

---

## What WinBrew CLI Does?

1. **Update** — `winbrew update` asks `https://api.winbrew.dev/v1/update` for a current, patch, or full-snapshot plan, then downloads the selected payload from the CDN and swaps it into place atomically.
2. **Search** — `winbrew search <query>` queries the local catalog. It does not need the network after the database is present.
3. **Install** — `winbrew install <id>` resolves the package, selects the best installer, verifies the payload, and hands execution to the appropriate engine.
4. **Repair** — `winbrew repair` replays committed package journals and rebuilds package state. Committed journals are authoritative; incomplete or malformed journals are not.
5. **Remove** — `winbrew remove <id>` cleans up WinBrew-owned files, shims, and registry entries.

## Supported Installer Families

| Type | Status | Notes |
| --- | --- | --- |
| MSI | Supported | Windows Installer packages routed through the MSI engine. |
| MSIX / AppX | Supported | Delegates install and remove to the Windows package APIs. |
| EXE family | Supported | Covers Exe, Inno, Nullsoft, WiX, and Burn. |
| Portable | Supported | Copies raw payloads into the managed install root. |
| ZIP | Supported | Archive-shaped payloads are unpacked before the final engine is selected. |
| Font | Supported | Installs fonts through the Windows font path. |
| PWA | Not routed yet | Present in the model, but intentionally not implemented. |

---

## Architecture at a Glance

The repository is split by responsibility.

- `infra/` contains the Go crawler and publisher. They ingest Winget and Scoop, normalize catalog data, and publish the daily snapshot.
- `crates/` contains the Rust runtime: models, database, core helpers, engines, app orchestration, CLI, UI, and Windows-only helpers.
- `docs/` captures the contracts that should stay stable: engine ownership, recovery policy, managed paths, and workspace structure.

A few details matter:

- The catalog is the canonical offline artifact for search and update.
- The refresh flow is API-driven through `https://api.winbrew.dev/v1/update`; the worker decides whether a client should stay current, apply patches, or download a full snapshot.
- Repair depends on committed journals as the recovery trail, not on live disk state.
- The engine layer keeps package-type routing data-driven so the app layer does not need a chain of hidden conditionals.

If you want the deeper map, start with [docs/index.md](docs/index.md), then read [docs/engines.md](docs/engines.md) and [docs/recovery-policy.md](docs/recovery-policy.md).

---

## FAQ

See [docs/faq.md](docs/faq.md) for answers about catalog freshness, offline usage, safety, proxy support, and more.

---

## License

Licensed under either of [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE) at your option.