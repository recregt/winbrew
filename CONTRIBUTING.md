# Contributing to WinBrew

This document outlines the architectural rules, schema ownership guidelines, and CI/CD pipelines for contributing to WinBrew.

For steps to set up the development environment, build, test, and run WinBrew locally from source, please read [docs/BUILD.md](docs/BUILD.md) first.

## Documentation Map

Before making structural changes, familiarize yourself with the internal documentation.

### Architecture and Policies

| Topic | Location |
| --- | --- |
| Full Documentation Index | [docs/index.md](docs/index.md) |
| Schema Ownership Rules | [docs/create-dependence.md](docs/create-dependence.md) |
| Runtime Directory Policy | [docs/managed-paths-policy.md](docs/managed-paths-policy.md) |
| Recovery Contracts | [docs/recovery-policy.md](docs/recovery-policy.md) |

### Infrastructure Components

| Component | Documentation |
| --- | --- |
| Crawler | [infra/crawler/README.md](infra/crawler/README.md) |
| Parser | [infra/parser/README.md](infra/parser/README.md) |
| Publisher | [infra/publisher/README.md](infra/publisher/README.md) |

## Schema Ownership

The WinBrew shared contract layout is strictly defined to prevent data inconsistency.

```text
WinBrew Shared Contract Layout
├── infra/parser/schema/catalog.sql     → Parser DDL (catalog.db)
├── crates/database/src/migration.rs    → Main DB DDL (installed packages)
├── crates/database/src/journal/        → Journal JSONL format + replay
└── crates/models/                      → Shared DTOs & validation layer
```

**Rule:** When changing persisted contracts, update the owning module AND root contract tests in the same pull request. Never add duplicate schema sources in tests, fixtures, or build scripts.

## CI Pipelines (Local and Remote)

While day-to-day testing commands are documented in the build guide, these pipelines ensure integration stability across the repository.

| Command | What it Runs |
| --- | --- |
| `task ci:rust:fast` | Pre-commit Rust lane (quick checks) |
| `task ci:parser` | Catalog parser validation |
| `task ci:docs` | Full workspace doc generation (incl. private) |
| `task ci:go:crawler` | Go crawler linting |
| `task ci:go:publisher` | Go publisher bundle flow checks |
| `task ci:smoke` | Build and smoke test CLI |
| `task ci:verify` | All-in-One: Runs crawler → publisher → parser → docs → rust → smoke |

## Git Hooks

This repository uses [Lefthook](https://github.com/evilmartians/lefthook) to enforce code quality and commit message standards automatically. The hook configuration lives in [`lefthook.yml`](lefthook.yml).

Install Lefthook once after cloning:

```powershell
lefthook install
```

### Hooks

| Hook | Runs | Trigger |
| --- | --- | --- |
| `commit-msg` | Validates commit message format via [`scripts/tasks/commit-msg.ps1`](scripts/tasks/commit-msg.ps1) | Every commit |
| `pre-commit` | `task ci:rust:fast`, `task ci:go:crawler` (on `infra/crawler/**`), `task ci:go:publisher` (on `infra/publisher/**`) | Every commit |
| `pre-push` | `task ci:rust`, `task ci:go:crawler` (on `infra/crawler/**`), `task ci:go:publisher` (on `infra/publisher/**`) | Every push |

`pre-commit` and `pre-push` hooks run in parallel. `pre-push` runs the full Rust test suite (`task ci:rust`) instead of the fast pre-commit lane.

### Commit Message Format

Commit messages are validated against the [Conventional Commits](https://www.conventionalcommits.org/) spec by `scripts/tasks/commit-msg.ps1`. The accepted pattern is:

```
<type>(<scope>)!: <description>
```

Valid types: `feat`, `fix`, `docs`, `style`, `refactor`, `perf`, `test`, `build`, `ci`, `chore`, `revert`

Examples:

```
feat(cli): rename binary to winbrew
fix(cleaner)!: remove legacy behavior
docs(contributing): add git hooks section
```

The `!` suffix denotes a breaking change. Scope is optional but encouraged.

## Code Style

### Rust

- `cargo fmt` and `cargo clippy` must produce clean output. **No warnings, no formatting diffs — CI will reject the PR automatically.**
- Crate layer ownership and dependency boundaries are strictly enforced. See [docs/create-dependence.md](docs/create-dependence.md) for the full ruleset. Key invariants:
  - `winbrew-app` must remain UI-free — no `UiSettings`, no spinner or prompt logic.
  - `winbrew-ui` is presentation-only.
  - The dependency graph flows in one direction: `bin → cli → app / core / database / ui`.

### Go

- Code must be formatted with `gofmt`.
- Crawler and publisher linting is checked via `task ci:go:crawler` and `task ci:go:publisher` respectively, and is also enforced in `pre-commit` and `pre-push` hooks for changed files under `infra/`.

## Catalog Bundle Pipeline

The catalog update process follows a strict flow:

`Crawler (Go) → Winget DB + JSONL Stream → Parser (Rust) → catalog.db + metadata.json → Publisher (Go) → R2/Cloudflare`

### Local End-to-End Test

To test the entire catalog ingestion pipeline locally, run the following script. It will stage files (`winget_source.jsonl`, `catalog.db`, and `metadata.json`) to the `staging/` folder.

```powershell
Set-Location infra/crawler; `
  go run ./cmd/crawler --winget-out ..\staging\winget_source.jsonl | `
  cargo run --manifest-path ..\..\Cargo.toml -p winbrew-infra-parser -- `
    --winget-jsonl ..\staging\winget_source.jsonl `
    --out ..\staging\catalog.db `
    --metadata ..\staging\metadata.json
```

### CI Deployment Workflow

**Workflow File:** [.github/workflows/catalog.yml](.github/workflows/catalog.yml)

| Trigger | Environment |
| --- | --- |
| Manual dispatch | Ubuntu |
| Scheduled | Every 6 hours |

**Required Secrets:**

* `R2_ACCESS_KEY_ID`
* `R2_SECRET_ACCESS_KEY`
* `CLOUDFLARE_ACCOUNT_ID`
* `CLOUDFLARE_API_TOKEN`
* `WINBREW_UPDATE_DB_ID`

**Optional Repo Variables:**

* `R2_BUCKET_NAME` (default: `winbrew-assets`)
* `CATALOG_PUBLIC_BASE_URL` (default: `https://cdn.winbrew.dev`)
* `WINBREW_UPDATE_DB_NAME` (default: `winbrew-update`)

## Exit Codes Reference

When contributing to infrastructure binaries, adhere to the following exit codes.

### Crawler

| Code | Meaning | Common Causes |
| --- | --- | --- |
| `0` | Success | Both sources completed, DB written |
| `1` | Failure | Config/fetch errors, retries exhausted, cancellation |

### Catalog Parser

| Code | Meaning | Common Causes |
| --- | --- | --- |
| `0` | Success | Bundle written successfully |
| `1` | Pipeline failure | SQLite/write/hash/runtime errors |
| `2` | Usage error | Missing args or invalid flags |

### Publisher

| Code | Meaning | Common Causes |
| --- | --- | --- |
| `0` | Success | Hash matched or upload succeeded |
| `1` | Failure | Missing R2 config, hash mismatch, upload failure |

## Notes

* **Documentation Publication:** Workspace rustdoc is generated via `task ci:docs` and deployed automatically from `target/doc` in the CI `doc` job to [winbrew.pages.dev](https://winbrew.pages.dev) (Docs.rs is not used).
* **Spellcheck:** Enforced in CI-only unless you have `typos` installed locally.
* **Task Alignment:** Keep local `task` commands synced with GitHub Actions workflows when adding or renaming pipeline steps.