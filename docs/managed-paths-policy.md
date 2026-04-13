# Managed Paths Policy

This document defines the directory contract for the WinBrew managed root.

The goal is to keep one clear rule set for which paths WinBrew owns under the
active root, when those paths are created, what data belongs there, and which
filesystem locations must remain outside the managed tree. This policy covers
the current runtime layout plus the MSI/MSIX forensic additions that we want to
standardize.

## 1. Root Ownership

The active WinBrew root is the directory that contains the runtime config,
package state, logs, cache, and managed package trees.

The current resolution order is:

- Default root: `%LOCALAPPDATA%\winbrew`
- Environment override: `WINBREW_PATHS_ROOT`
- Persisted config override: `[paths].root`

The process should treat the resolved root as a single managed tree for the
current execution. WinBrew should not split one installation across multiple
roots at the same time.

Important distinction:

- `WINBREW_PATHS_ROOT` chooses where the config file is loaded from and where
  the active root starts.
- `[paths].root` is the persisted root value that the runtime uses once config
  is loaded.

## 2. Current Managed Tree

The current tree is intentionally small and lazy-created. WinBrew creates only
the directories it needs for the current command or startup phase.

```text
%LOCALAPPDATA%\winbrew
├── packages/
│   └── <package-name>/              # Package install root owned by WinBrew
└── data/
    ├── db/
    │   ├── winbrew.db               # Primary application database
    │   └── catalog.db               # Catalog database when present
    ├── pkgdb/
    │   └── <package-key>/
    │       └── journal.jsonl        # Per-package recovery journal
    ├── logs/
    │   └── winbrew.log              # Process-wide tracing output
    ├── cache/                       # Downloaded installers and staging files
    └── winbrew.toml                 # Persisted runtime configuration
```

Current ownership rules:

- `packages/` holds package install roots created by WinBrew-managed installs.
- `data/db/` holds SQLite state and catalog state.
- `data/pkgdb/` holds recovery journals and other package-scoped recovery
  evidence.
- `data/logs/` holds process logs.
- `data/cache/` holds downloaded payloads and other managed temporary artifacts
  that are meant to survive within the root during a session.
- `data/winbrew.toml` is the persisted configuration file.

## 3. Directory Creation Rules

WinBrew creates directories on demand rather than pre-populating the entire
tree on first launch.

Creation triggers are:

- `data/logs/` when the process logging subsystem initializes.
- `data/db/` when the database pool resolves or opens the backing SQLite files.
- `data/pkgdb/` when journal writers or recovery code need package journals.
- `packages/<package-name>/` when an install workflow prepares the install
  target.
- `data/cache/` when the download or staging pipeline needs cached files.
- `data/winbrew.toml` when config commands persist settings.

This laziness is deliberate. It keeps first-run overhead low and avoids creating
state that a user may never need.

## 4. MSI/MSIX Forensic Additions

MSI and MSIX are not filesystem-copy engines in the same sense as ZIP or
portable packages. The managed root should therefore store evidence and
metadata, not pretend that WinBrew authored every file on disk.

The target structure for portable-launcher and forensic data is:

```text
%LOCALAPPDATA%\winbrew
├── shims/
│   └── <package-key>/
│       └── <launcher shims>
├── data/
│   ├── pkgdb/
│   │   └── <package-key>/
│   │       └── journal.jsonl
│   └── logs/
│       └── packages/
│           └── <package-key>/
│               └── <engine-specific log files>
```

Expected behavior for the added MSI/MSIX and portable-shim features:

- When an install emits package-scoped evidence, it should use
  `data/logs/packages/<package-key>/`.
- MSI should place its verbose Windows Installer log there and record the log
  path in the package journal.
- MSI should also capture a post-install inventory snapshot and keep the
  snapshot reference in the journal or receipt trail.
- MSIX should store identity evidence there only when needed; it does not need
  a large verbose log file by default.
- Portable packages may materialize shims under `shims/<package-key>/` so the
  package can expose launcher entry points without polluting the package install
  root.
- The package journal remains the canonical per-package recovery trail; the
  external installer log is evidence, not the source of truth.

For MSI, the journal should describe the lifecycle in witness terms:

- intent to install
- external installer log reference
- post-install inventory snapshot reference
- completion or failure outcome

For MSIX, the journal should usually only need identity and outcome data:

- intent to install
- package identity / full name
- completion or failure outcome

## 5. Unmanaged Paths

The following locations are intentionally outside the managed root contract:

- `%TEMP%\winbrew` and any other transient installer workspace under the system
  temp directory
- Windows Installer cache and other OS-managed MSI/MSIX artifacts
- registry hives and uninstall roots that Windows owns directly
- downloaded source locations that WinBrew does not copy into `data/cache/`

These paths may be read during diagnostics or recovery, but WinBrew should not
present them as part of the owned root tree.

## 6. Operational Summary

- `%LOCALAPPDATA%\winbrew` is the default root.
- `WINBREW_PATHS_ROOT` and `[paths].root` can redirect the active root.
- `packages/`, `data/db/`, `data/pkgdb/`, `data/logs/`, and `data/cache/` are
  owned by WinBrew.
- `shims/` is the planned home for package-scoped portable launcher shims.
- `data/pkgdb/<package-key>/journal.jsonl` is the per-package recovery trail.
- `data/logs/packages/<package-key>/` is the target home for package-scoped MSI
  and MSIX evidence.
- Temporary workspaces under `%TEMP%\winbrew` stay unmanaged.
- `bin/` is intentionally not part of the current plan because WinBrew does not
  currently have a second managed binary to place there.

This policy is intentionally conservative. It keeps durable state, recovery
history, and transient installer work in separate lanes so future MSI/MSIX
forensic features stay predictable.
