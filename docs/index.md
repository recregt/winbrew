# WinBrew Docs Index

This page is the entry point for the workspace documentation map.

Use it when you want to answer one of these questions:

- What crate owns a given concept?
- Which docs describe runtime contracts versus contributor workflow?
- Where should a new contributor start reading before making a change?

## Start Here

- [README](../README.md) for the user-facing overview and local build path.
- [Contributing](../CONTRIBUTING.md) for setup, task commands, and validation.
- [Create Dependency Boundaries](create-dependence.md) for object ownership and wiring rules.

## Runtime Contracts

- [Managed Paths Policy](managed-paths-policy.md) for the owned directory tree.
- [Recovery Policy](recovery-policy.md) for journal, SQLite, and disk authority.

## Engine Strategy

- [Engine Roadmap and Ownership](engines.md) for supported package types, ownership boundaries, and journal strategy.

## Workspace Architecture

- [winbrew-models](../crates/models/src/lib.rs) for typed model contracts.
- [winbrew-core](../crates/core/src/lib.rs) for shared filesystem, hash, network, and path helpers.
- [winbrew-database](../crates/database/src/lib.rs) for persistence and database access.
- [winbrew-engines](../crates/engines/src/lib.rs) for engine dispatch and platform-specific installers.
- [winbrew-app](../crates/app/src/lib.rs) for workflow orchestration.
- [winbrew-cli](../crates/cli/src/lib.rs) for command parsing, dispatch, and terminal wiring.
- [winbrew-ui](../crates/ui/src/lib.rs) for terminal presentation state.
- [winbrew-windows](../crates/windows/src/lib.rs) for Windows-only platform helpers.

## Pipeline Docs

- [infra/crawler](../infra/crawler/README.md) for source crawling.
- [infra/parser](../infra/parser/README.md) for catalog bundle parsing.
- [infra/publisher](../infra/publisher/README.md) for bundle publishing.

## Validation

- [Taskfile](../Taskfile.yml) for local task entry points.
- [Rust CI script](../scripts/ci/check-rust.ps1) for the Windows Rust lane.
- [Parser CI script](../scripts/ci/check-rust-parser.ps1) for the catalog parser lane.
- [.github/workflows/main.yml](../.github/workflows/main.yml) for the full CI graph.

## Reading Order

1. Read the README for the user-facing summary.
2. Read `docs/engines.md` for package-type ownership and engine routing.
3. Read `docs/create-dependence.md` for ownership and wiring.
4. Read the policy docs for runtime contracts.
5. Read the crate-level docs for the layer you plan to change.

The docs are intentionally split by responsibility. If a page starts to repeat another page, the repeated material should usually move to the more specific owner.