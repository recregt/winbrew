# Managed Paths Policy

This document defines the directory contract for the WinBrew managed root.

The goal is to keep one clear rule set for which paths WinBrew owns under the
active root, when those paths are created, what data belongs there, and which
filesystem locations must remain outside the managed tree. Engine-specific
evidence placement and launcher-shim behavior live in [Engine Roadmap and Ownership](engines.md).

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
├── bin/
│   └── 7zip/                        # Optional local 7-Zip runtime bootstrap
├── packages/
│   └── <package-name>/              # Package install root owned by WinBrew
├── shims/
│   └── <package-key>/               # Package command shims
└── data/
    ├── db/
    │   ├── winbrew.db               # Primary application database
    │   └── catalog.db               # Catalog database when present
    ├── pkgdb/
    │   └── <package-key>/
    │       └── journal.jsonl        # Per-package recovery journal
    ├── logs/
    │   ├── winbrew.log              # Process-wide tracing output
    │   └── packages/                # Package-scoped log roots when needed
    ├── cache/                       # Downloaded installers and staging files
    └── winbrew.toml                 # Persisted runtime configuration
```

Current ownership rules:

- `packages/` holds package install roots created by WinBrew-managed installs.
- `shims/` holds the managed command shims that install, remove, and repair
  publish or clean up.
- `data/db/` holds SQLite state and catalog state.
- `data/pkgdb/` holds recovery journals and other package-scoped recovery
  evidence.
- `data/logs/` holds process logs.
- `data/logs/packages/` holds package-scoped log directories when a workflow
  needs them.
- `data/cache/` holds downloaded payloads and other managed temporary artifacts
  that are meant to survive within the root during a session.
- `data/winbrew.toml` is the persisted configuration file.
- `bin/7zip/` holds the optional locally bootstrapped 7-Zip runtime, not a
  general-purpose binary directory.

## 3. Directory Creation Rules

WinBrew creates directories on demand rather than pre-populating the entire
tree on first launch.

Creation triggers are:

- `data/logs/` when the process logging subsystem initializes.
- `data/logs/packages/` when package-scoped logging or evidence needs it.
- `data/db/` when the database pool resolves or opens the backing SQLite files.
- `data/pkgdb/` when journal writers or recovery code need package journals.
- `packages/<package-name>/` when an install workflow prepares the install
  target.
- `shims/` when install, remove, or repair needs to publish or clean command
  shims.
- `data/cache/` when the download or staging pipeline needs cached files.
- `data/winbrew.toml` when config commands persist settings.
- `bin/7zip/` when the optional 7-Zip runtime bootstrap is approved and
  published.

This laziness is deliberate. It keeps first-run overhead low and avoids creating
state that a user may never need.

## 4. Engine-Specific Evidence

This policy does not define package-type behavior in detail.

Use [Engine Roadmap and Ownership](engines.md) for the package-type matrix,
engine-owned evidence placement, and journal interpretation.

The generic rule this policy keeps is simple: the managed root owns the durable
package journal under `data/pkgdb/` and the normal runtime tree under
`packages/`, `data/db/`, `data/logs/`, and `data/cache/`.

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
- `shims/` is owned by WinBrew and stores command shims derived from committed
  package metadata.
- `data/pkgdb/<package-key>/journal.jsonl` is the per-package recovery trail.
- Engine-specific evidence placement and launcher-shim conventions live in
  [Engine Roadmap and Ownership](engines.md).
- Temporary workspaces under `%TEMP%\winbrew` stay unmanaged.
- `bin/7zip/` is reserved for the optional local 7-Zip runtime and is not a
  general-purpose binary drop location.

This policy is intentionally conservative. It keeps durable state, recovery
history, and transient installer work in separate lanes so future MSI/MSIX
forensic features stay predictable.
