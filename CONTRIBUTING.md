# Contributing to WinBrew

> **WinBrew** uses **[go-task](https://taskfile.dev/)** and **[Lefthook](https://lefthook.dev/)** 
> for development workflow management.

## Quick Start

### Prerequisites & Setup

```powershell
# Install development tools
task tools:install-lefthook
task tools:install-nextest  
task tools:install-golangci-lint

# Initialize git hooks
lefthook install
```

### First Run

```powershell
# Run locally (uses target\winbrew-dev, won't pollute your system)
task dev:run -- doctor

# Or install something
task dev:run-release -- install firefox
```

## 📚 Documentation Map

### Arch

| Topic | Location |
|-------|----------|
| **Full Documentation Index** | [docs/index.md](docs/index.md) |
| **Schema Ownership Rules** | [docs/create-dependence.md](docs/create-dependence.md) |
| **Runtime Directory Policy** | [docs/managed-paths-policy.md](docs/managed-paths-policy.md) |
| **Recovery Contracts** | [docs/recovery-policy.md](docs/recovery-policy.md) |

### Infra

| Component | Documentation |
|-----------|---------------|
| **Crawler** | [infra/crawler/README.md](infra/crawler/README.md) |
| **Parser** | [infra/parser/README.md](infra/parser/README.md) |
| **Publisher** | [infra/publisher/README.md](infra/publisher/README.md) |

## Schema Ownership

```
WinBrew Shared Contract Layout
├── infra/parser/schema/catalog.sql     → Parser DDL (catalog.db)
├── crates/database/src/migration.rs    → Main DB DDL (installed packages)
├── crates/database/src/journal/        → Journal JSONL format + replay
└── crates/models/                      → Shared DTOs & validation layer
```

**Rule:** When changing persisted contracts, update the owning module AND root contract tests in the same PR. Never add duplicate schema sources in tests/fixtures/build scripts.

## Development Tasks

### Code Quality (Pre-commit)

| Command | Purpose |
|---------|---------|
| `task check` | Format code (`cargo fmt`) |
| `task check:clippy` | Lint code (`cargo clippy`) |
| `task check:doc` | Generate docs (warnings = errors) |

### Testing

| Command | Scope |
|---------|-------|
| `task test` | Standard Rust tests |
| `task test:nextest` | Rust tests with nextest runner |

### CI Pipelines (Local)

| Command | What it Runs |
|---------|--------------|
| `task ci:rust:fast` | **Pre-commit** Rust lane (quick checks) |
| `task ci:rust` | **Pre-push/CI** Full Rust suite (nextest) |
| `task ci:parser` | Catalog parser validation |
| `task ci:docs` | Full workspace doc generation (incl. private) |
| `task ci:go:crawler` | Go crawler linting |
| `task ci:go:publisher` | Go publisher bundle flow checks |
| `task ci:smoke` | Build + smoke test CLI |

### All-in-One

```powershell
# Run everything (crawler → publisher → parser → docs → rust → smoke)
task ci:verify
```

### Utilities

| Command | Description |
|---------|-------------|
| `task dev:run -- <args>` | Local dev run (debug mode) |
| `task dev:run-release -- <args>` | Local dev run (release mode) |
| `task dev:clean` | Clean dev environment |

> 💡 **Tip:** All dev runs use `target\winbrew-dev` via `WINBREW_PATHS_ROOT`, keeping config/logs/dbs inside the repo.

## 🔄 Catalog Bundle Pipeline

### Overview

```
Crawler (Go) → Winget DB + JSONL Stream → Parser (Rust) → catalog.db + metadata.json → Publisher (Go) → R2/Cloudflare
```

### Local End-to-End Test

```powershell
Set-Location infra/crawler; `
  go run ./cmd/crawler --winget-out ..\staging\winget_source.jsonl | `
  cargo run --manifest-path ..\..\Cargo.toml -p winbrew-infra-parser -- `
    --winget-jsonl ..\staging\winget_source.jsonl `
    --out ..\staging\catalog.db `
    --metadata ..\staging\metadata.json
```

**Output:** Stages files to `staging/` folder:
- `winget_source.jsonl` - Raw Winget data
- `catalog.db` - Merged catalog snapshot  
- `metadata.json` - Bundle metadata/hash

### CI Deployment

**Workflow:** [.github/workflows/catalog.yml](.github/workflows/catalog.yml)

| Trigger | Environment |
|---------|-------------|
| Manual dispatch | Ubuntu |
| Scheduled | Every 6 hours |

**Required Secrets:**
- `R2_ACCESS_KEY_ID`
- `R2_SECRET_ACCESS_KEY`
- `CLOUDFLARE_ACCOUNT_ID`

**Optional Repo Variable:**
- `R2_BUCKET_NAME` (default: `winbrew-assets`)

## 📊 Exit Codes Reference

### Crawler

| Code | Meaning | Common Causes |
|------|---------|---------------|
| `0` | ✅ Success | Both sources completed, DB written |
| `1` | ❌ Failure | Config/fetch errors, retries exhausted, cancellation |

### Catalog Parser

| Code | Meaning | Common Causes |
|------|---------|---------------|
| `0` | ✅ Success | Bundle written successfully |
| `1` | ❌ Pipeline failure | SQLite/write/hash/runtime errors |
| `2` | ⚠️ Usage error | Missing args or invalid flags |

### Publisher

| Code | Meaning | Common Causes |
|------|---------|---------------|
| `0` | ✅ Success | Hash matched or upload succeeded |
| `1` | ❌ Failure | Missing R2 config, hash mismatch, upload failure |

## 📝 Notes

- **Documentation Publication:** Workspace rustdoc via `task ci:docs`, deployed from `target/doc` in CI `doc` job to [winbrew.pages.dev](https://winbrew.pages.dev) (Docs.rs not used).
- **Spellcheck:** CI-only unless Typos installed locally
- **Task Alignment:** Keep local tasks synced with GitHub Actions workflow when adding/renaming
