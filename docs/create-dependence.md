# Create Dependency Boundaries

This document defines how context construction, UI settings, and resource-creation helpers are owned across the workspace.

The goal is to keep the app layer focused on business state and execution, keep presentation state in the CLI layer, and make creator/factory dependencies predictable instead of implicit.

This note is the companion to `docs/recovery-policy.md`. That document defines recovery behavior. This document defines object creation and dependency wiring.

## Scope

This document covers:

- runtime context construction
- CLI boot wiring
- UI settings ownership
- infrastructure builders such as database connections, temp roots, and network clients
- domain helpers that act like creators or resolvers

This document does not cover:

- generic filesystem `create_*` calls unless they affect layer ownership
- unrelated crate moves
- new runtime behavior
- naming churn for helpers that are already correctly owned

## Current Flow

The current startup path is:

1. `crates/bin/src/main.rs` parses CLI arguments and calls `winbrew_cli::run_app`.
2. `crates/cli/src/lib.rs` loads configuration, builds the CLI command context, initializes logging and the database, then dispatches the command.
3. `CommandContext` wraps `AppContext` plus `UiSettings` and can construct `Ui` for command handlers.
4. Command handlers obtain `Ui` from `ctx.ui()` and call into app helpers.
5. App helpers stay UI-free and can call storage/core/network helpers as needed.

The important ownership rule is simple:

- `AppContext` belongs to `winbrew-app`.
- `Ui` belongs to `winbrew-ui`.
- `UiSettings` belongs to `winbrew-ui` and is stored inside the CLI-owned `CommandContext` wrapper.
- `CommandContext` is a CLI-owned wrapper that combines the app context and presentation settings.

## Ownership Rules

### App Layer

The app crate owns runtime and business context.

It may construct:

- `AppContext`
- recovery and repair plans
- install and repair resolution targets
- business-level result types

It must not own:

- `UiSettings`
- `Ui`
- prompt logic
- spinner logic
- command-specific presentation state

### CLI Layer

The CLI crate owns presentation and orchestration.

It may construct:

- `CommandContext`
- `Ui`
- command-level observers and adapters
- confirmation flow and package-selection flow

It must not broaden app-owned context with presentation concerns.

### UI Layer

The UI crate owns rendering state and presentation configuration.

It may construct:

- `Ui`
- `UiBuilder`
- `UiSettings`

It must remain presentation-only.

### Core, Storage, and Windows Layers

These crates own shared infrastructure or platform-specific resources.

- `winbrew-core` owns shared infrastructure helpers such as temp workspace and network primitives.
- `winbrew-storage` owns persistence helpers and database connection access.
- `winbrew-windows` owns platform-specific wrappers and platform resource creation.

These crates should expose factory helpers for their own resources, but they should not absorb presentation state.

## Creator And Dependency Matrix

| Creator or helper | Owner crate | Inputs | Returns | Typical callers | Boundary rule |
| --- | --- | --- | --- | --- | --- |
| `AppContext::from_config_with_verbosity` | `winbrew-app` | `database::Config`, verbosity | `AppContext` | CLI boot wiring, tests | App-owned runtime context only |
| `CommandContext::from_config_with_verbosity` | `winbrew-cli` | `database::Config`, verbosity | `CommandContext` | CLI boot wiring, CLI tests | CLI-owned wrapper around app context and UI settings |
| `CommandContext::ui` | `winbrew-cli` | internal UI settings | `Ui` | CLI command handlers | CLI-owned presentation factory |
| `Ui::new` / `UiBuilder::new` | `winbrew-ui` | `UiSettings` | `Ui` | CLI command handlers | Presentation only |
| `install::download::build_client` | `winbrew-app` via `winbrew-core` network helper | user agent, network config | HTTP client | install and repair flows | Infrastructure helper, not UI-aware |
| `temp_workspace::build_temp_root` | `winbrew-core` | package name, version | temp path | install and repair flows | Shared infrastructure helper |
| `database::get_conn` | `winbrew-storage` | current database state | DB connection | app and bootstrap code | Persistence boundary |
| `database::get_catalog_conn` | `winbrew-storage` | current catalog database state | catalog DB connection | repair and install resolution | Persistence boundary |
| `repair::build_repair_plan` | `winbrew-app` | `HealthReport`, packages root | `RepairPlan` | CLI repair orchestration | Pure planning helper |
| `repair::resolve_file_restore_target` | `winbrew-app` | package name, chooser callback | resolution enum | CLI repair orchestration | Decision helper, no UI ownership |
| `install::run` | `winbrew-app` | `AppContext`, package ref, observer | install outcome | CLI install and repair | App-owned execution with callback boundary |

## Baseline Rules For New Code

When you need to create a new object or resource, use the following rule set:

1. Put the constructor in the crate that owns the type.
2. Keep presentation state out of app-owned constructors.
3. If a caller needs both app context and UI settings, wrap them in a CLI-owned context instead of expanding `AppContext`.
4. If a helper creates a network client, temp path, DB connection, or platform resource, keep that helper with the crate that owns the resource.
5. If a command needs interaction, pass behavior through an observer or callback instead of letting app code create UI objects.

## Current Accepted Pattern

The accepted shape is:

- `Config` is loaded in CLI bootstrap.
- `CommandContext` is created in CLI bootstrap.
- `CommandContext` dereferences to `AppContext` for app calls.
- `Ui` is created inside command handlers only.
- app code requests infrastructure through owned helpers or callback boundaries.

This pattern keeps command presentation at the top of the stack and keeps app helpers reusable in tests and other callers.

## Create Categories

### 1. Context Constructors

These constructors create long-lived runtime context objects.

- `AppContext::from_config_with_verbosity`
- `CommandContext::from_config_with_verbosity`

Rules:

- app constructors may not accept `UiSettings`
- CLI constructors may combine app context with presentation state
- tests should use the lowest context that matches the layer being tested

### 2. UI Constructors

These constructors create presentation objects.

- `Ui::new`
- `UiBuilder::new`
- `UiBuilder::with_writer`

Rules:

- only CLI and UI tests should instantiate them directly
- app code should never construct `Ui`

### 3. Infrastructure Builders

These constructors create reusable resources or handles.

- `temp_workspace::build_temp_root`
- `install::download::build_client`
- database connection helpers
- Windows-specific platform wrappers

Rules:

- keep them in the owning crate
- return only infrastructure types, not presentation state
- avoid pushing command-specific decisions into them

### 4. Domain Resolvers And Decision Helpers

These helpers create decision payloads rather than UI state.

- repair plan builders
- package resolution helpers
- file-restore target resolution

Rules:

- keep the resolver pure where possible
- use callbacks or observer traits for ambiguous selection
- return typed decision payloads instead of prompting directly

## Practical Guardrails

- `AppContext` should remain UI-free.
- `CommandContext` should be the only CLI-facing wrapper for `AppContext` plus `UiSettings`.
- `winbrew-cli` should not re-export app internals or presentation types that would let callers bypass the ownership model.
- creator helpers should stay close to the layer that owns the resource they create.
- if a helper starts making presentation decisions, move that decision back up to CLI.

## Implementation Actions

When auditing or changing this area, follow these steps:

1. Trace the boot path from binary entrypoint to command execution.
2. Classify every constructor/helper as app-owned, CLI-owned, UI-owned, or infrastructure-owned.
3. Move any presentation settings out of app-owned types.
4. Wrap mixed ownership in a CLI-owned context instead of expanding app context.
5. Keep observer/callback boundaries for interactive flows.
6. Update tests to match the owner of the object being created.
7. Verify the crate graph still flows in one direction: bin -> cli -> app/core/storage/ui.

## Non-Goals

- This document does not rename every `create_*` helper in the codebase.
- This document does not move crates around.
- This document does not introduce new runtime abstractions.
- This document does not change repair, install, or doctor behavior.

## Verification Checklist

- `crates/app` does not depend on `winbrew-ui`
- `UiSettings` is not re-exported from `winbrew-cli` and remains a `winbrew-ui` type stored inside `CommandContext`
- `CommandContext` is the CLI envelope around `AppContext`
- command handlers obtain `Ui` from `CommandContext`
- infrastructure builders stay in their owning crates
- tests reflect the same boundary as production code

The intended outcome is a clean dependency graph where context creation, presentation, and infrastructure creation each have a single owner.