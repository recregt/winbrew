# Engine Roadmap and Ownership

This page is the canonical roadmap for WinBrew package-type support. 

Note that, the engine layer is typically dependent on Windows Crate for the Windows platform helper API and implementation details, see [crates/windows/README.md](../crates/windows/README.md).

Use it to answer these questions:

- Which package types are supported today?
- Which package types are scaffolded but not routed yet?
- What does WinBrew own versus what does Windows own?
- How does the engine registry choose an implementation?
- What is the current journal model for engine-specific recovery data?

If this page ever disagrees with a narrower policy document, this page wins for engine-specific behavior.

## Canonical Location

The canonical copy lives here in `docs/engines.md`.

That keeps the design discussion close to the rest of the workspace documentation map while leaving crate docs focused on API surface and module boundaries.

Use this page to track support status and routing decisions. Use the Windows README when you need the caller-facing helper surface or the concrete OS boundary behavior.

`crates/engines/src/lib.rs` should stay a crate facade, not a full roadmap.

## Current Support Matrix

| Package type | Engine kind | Current status | Ownership pattern | Notes |
| --- | --- | --- | --- | --- |
| `InstallerType::Msi` | `EngineKind::Msi` | Supported on Windows | Windows-delegated, WinBrew-coordinated | Scans MSI inventory first, runs `msiexec`, records product code, upgrade code, scope, registry keys, shortcuts, and inventory snapshot. |
| `InstallerType::Msix` | `EngineKind::Msix` | Supported on Windows | Windows-delegated, WinBrew-coordinated | Delegates install/remove to Windows App Installer / package APIs and records package identity metadata. |
| `InstallerType::Zip` | `EngineKind::Zip` | Supported | WinBrew-owned filesystem engine | ZIP is the archive front door today. The same dispatcher already carries the `ArchiveKind` family, and ZIP/Tar backends are implemented there. Remove is plain directory cleanup. |
| `InstallerType::Portable` | `EngineKind::Portable` | Supported | WinBrew-owned filesystem engine | Copies raw payloads into a staging tree, then replaces the target install directory. Raw-only fallback; archive-shaped payloads route through the archive dispatcher instead of Portable. Remove is plain directory cleanup. |
| `InstallerType::Font` | `EngineKind::Font` | Supported on Windows | Windows-delegated, WinBrew-coordinated | Installs raw font payloads into the per-user Windows fonts directory and removes the copied file on uninstall. |
| `InstallerType::Exe` | `EngineKind::NativeExe` | Supported on Windows | Windows-delegated, WinBrew-coordinated | v1 covers the native-exe family (`Exe`, `Inno`, `Nullsoft`, `Burn`). Switches are parsed literally and duplicate entries are rejected. `Pwa` remains scaffolded. |

### Practical split

- WinBrew-owned engines are the filesystem engines: `Zip` handles archive extraction, and `Portable` handles raw payload copying.
- Windows-delegated engines are the OS-backed engines: `Msi`, `Msix`, `NativeExe`, and `Font`.
- `NativeExe` is already supported on Windows and follows the same Windows-delegated ownership pattern as `Msi` and `Msix`.
- `Font` is supported on Windows as a per-user font engine; `Pwa` remains explicitly out of scope.

## How Routing Works

The engine layer does not branch on package kind directly. It resolves an installer into an engine through a descriptor table.

The relevant layers are:

- `crates/models/src/install/installer.rs` for `InstallerType`
- `crates/models/src/install/engine.rs` for `EngineKind`
- `crates/engines/src/lib.rs` for the `PackageEngine` trait and installer-kind to engine-kind conversion
- `crates/engines/src/registry.rs` for the descriptor table and runtime selection

Current routing rules:

- `InstallerType::Msi` resolves to `EngineKind::Msi`.
- `InstallerType::Msix` resolves to `EngineKind::Msix`.
- `InstallerType::Exe`, `InstallerType::Inno`, `InstallerType::Nullsoft`, and `InstallerType::Burn` resolve to `EngineKind::NativeExe`.
- `InstallerType::Font` resolves to `EngineKind::Font`.
- `InstallerType::Zip` resolves to `EngineKind::Zip`.
- `InstallerType::Portable` resolves to `EngineKind::Portable` for raw payloads.
- `InstallerType::Pwa` is still not routable and returns an unsupported-type error.
- Archive-shaped installers are classified into `ArchiveKind` and routed through the archive dispatcher; ZIP and Tar are implemented today, while 7z and rar still hit the generic backend-unavailable error.
- Portable installers whose URL looks like an archive are routed away from Portable and into the archive path.
- The archive descriptor must stay before portable in the registry table.

The registry is the place to keep that ordering logic visible. The selection should remain data-driven rather than a chain of hidden conditionals.

## Ownership Boundaries

### WinBrew-owned execution

`Zip` and `Portable` are the clearest owned engines.

WinBrew performs the full install/remove workflow on disk:

- downloads or stages the payload
- extracts ZIP or Tar payloads, or copies raw files
- replaces the install directory atomically where possible
- removes the install tree directly on uninstall

These engines do not depend on the Windows Installer service or App Installer to complete their work.

### Windows-delegated execution

`Msi` and `Msix` are coordination engines rather than pure filesystem engines.

WinBrew owns the orchestration and the recorded metadata, but Windows owns the final installation/removal action:

- MSI uses `msiexec`, pre/post inventory capture, uninstall registry lookup, and engine metadata recording.
- MSIX delegates to the Windows package APIs and records the package full name plus install scope.

In both cases, WinBrew should treat the OS as the execution authority and itself as the observer, normalizer, and persistence layer.

### Out of scope

`Pwa` is not supported.

The project still needs to decide whether it will become:

- a WinBrew-owned file placement engine,
- a Windows-delegated execution wrapper,
- or a narrow adapter around existing OS behavior.

Until that decision is made, the registry should keep rejecting it rather than pretending it is implemented.

## Journal Model

The current recovery trail is package-scoped, not a single monolithic journal for the whole workspace.

The storage layer writes and replays per-package journals under `data/pkgdb/<package-key>/journal.jsonl`:

- `JournalWriter::open_for_package_in` and `JournalReader::committed_paths_in` derive paths from `ResolvedPaths`.
- `JournalReader::read_committed` reads a committed journal stream.
- `JournalReader::read_committed_package` turns a committed journal into a replayable package record.
- Doctor scans those package journals to classify recovery issues.
- Repair replays committed package journals before it handles the remaining recovery groups.

Current authority rules:

- committed journal beats SQLite for rebuilds
- incomplete or malformed journals are never authoritative
- SQLite remains the runtime index
- disk remains the truth for file content checks

This means WinBrew does not currently need a central journal for all package types.

If a future feature needs cross-package atomicity, the better next step is an index or aggregator on top of the existing per-package journals, not a replacement of the current journal format.

## What Should Move Here

Engine-specific details should live here instead of being repeated in policy docs.

The following topics belong here:

- supported package type matrix
- WinBrew-owned versus OS-delegated execution
- descriptor-table routing rules
- package-scoped journal shape and recovery trail interpretation
- future engine additions and their ownership decisions

Policy docs should remain policy docs. They can point back here, but they should not restate the engine matrix in detail.

## Verification Strategy

Engine behavior should be verified in two layers.

- Unit tests stay close to the implementation in `crates/engines/src/...` and cover local parsing helpers, routing helpers, and invariants that only need module-level visibility.
- Integration tests live under `crates/engines/tests/` and exercise the public crate surface from the outside.
- Shared fixtures should live in `tests/common/mod.rs` so test cases stay short and focused.
- Table-driven scenarios are the default shape for engine routing and deployment-kind coverage; use small structs with explicit `description` fields when the assertion needs context.
- Keep targeted smoke tests for public adapter visibility, such as the MSIX surface, separate from the routing matrices.
- Add edge-case cases when they protect a real contract, such as Unicode paths, base URLs, or the distinction between archive-shaped and raw portable installers.

The current integration test command for this crate is:

```powershell
cargo test -p winbrew-engines --tests
```

If the matrix grows enough to justify extra generators or property-based coverage, add them later. The default should stay simple unless the test data becomes genuinely repetitive.

## Next Implementation Target

The nearest follow-up work is hardening the font backend and finishing any remaining native-exe edge cases.

Why it is the next obvious candidate:

- the v1 family route now exists for native executables and the separate font engine is now wired in
- `Pwa` remains explicitly out of scope
- both OS-backed install paths are intentionally conservative about switch and cleanup behavior

The implementation plan for that follow-up should cover:

- richer installer-family handling inside the native-exe backend
- deeper font validation, registration, and removal coverage
- tests for remove/recovery behavior using the engine-specific metadata and cleanup contract

## Related Files

- [crates/engines/src/lib.rs](../crates/engines/src/lib.rs)
- [crates/engines/src/registry.rs](../crates/engines/src/registry.rs)
- [crates/engines/src/windows/native/mod.rs](../crates/engines/src/windows/native/mod.rs)
- [crates/engines/src/windows/native/msi.rs](../crates/engines/src/windows/native/msi.rs)
- [crates/engines/src/windows/api/msix/mod.rs](../crates/engines/src/windows/api/msix/mod.rs)
- [crates/engines/src/filesystem/archive/zip/install.rs](../crates/engines/src/filesystem/archive/zip/install.rs)
- [crates/engines/src/filesystem/portable/install.rs](../crates/engines/src/filesystem/portable/install.rs)
- [crates/database/src/journal/mod.rs](../crates/database/src/journal/mod.rs)
- [crates/database/src/journal/replay.rs](../crates/database/src/journal/replay.rs)
- [crates/app/src/operations/doctor/scan/journal.rs](../crates/app/src/operations/doctor/scan/journal.rs)
- [crates/app/src/operations/repair.rs](../crates/app/src/operations/repair.rs)
