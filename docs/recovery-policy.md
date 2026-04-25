# Recovery Policy

This document defines how doctor and future repair tooling decide between SQLite, committed journals, and disk state.

The goal is to keep one clear rule set for rebuilding package state, classifying conflicts, and deciding when to ask the user for confirmation.

Engine-specific evidence placement and journal payload conventions are documented in [Engine Roadmap and Ownership](engines.md). This page stays focused on recovery authority and repair policy.

## 1. Authority Hierarchy

The system uses different sources of truth for different jobs:

- Committed journal is authoritative for rebuilding package state during recovery.
- SQLite is authoritative for normal runtime queries and the live package index.
- Incomplete or malformed journals are not authoritative. They can only be resumed if the file is still recoverable, or discarded if they are not.
- Disk is the source of truth for file content verification. It tells us whether the expected file exists and whether its hash matches, but it does not decide package state.

The practical rule is simple:

- For rebuilds, prefer committed journal over SQLite.
- For day-to-day reads, trust SQLite.
- For file content checks, trust disk.

## 2. Conflict Classes

Doctor should classify state problems into a small number of clear buckets.

| Case | Meaning | Default severity | Repair rule |
| --- | --- | --- | --- |
| SQLite exists, journal is missing | Recovery trail missing | Warning | The package is not broken, but its recovery history is incomplete. Keep the package record and report reduced recoverability. |
| Journal exists, SQLite is missing | Incomplete install | Error if the journal is committed | Treat the committed journal as a replay candidate and rebuild the SQLite rows from it. |
| SQLite and journal both exist, but disagree | Conflict | Error | Committed journal wins for reconstruction, but the user must confirm before SQLite is overwritten. |
| Disk differs from the stored inventory | Disk drift | Error | Do not repair package state from disk. Recommend reinstall or restore instead. |

Notes:

- "SQLite exists, journal is missing" does not mean the package is broken. It only means we lost the recovery trail.
- "Journal exists, SQLite is missing" is only actionable when the journal is committed. A committed journal can be replayed into a fresh transaction.
- "Conflict" is a policy decision, not an automatic overwrite. The repair path must show the difference and ask for explicit approval.
- Disk drift is a separate category from state drift. Missing files and hash mismatches are content problems, not database problems.

### Diagnostic to Recovery Mapping

The current doctor implementation maps diagnostics into recovery findings as follows:

| Diagnostic code(s) | Recovery issue | Action group | Notes |
| --- | --- | --- | --- |
| `missing_install_directory`, `install_directory_not_a_directory`, `install_directory_permission_denied`, `install_directory_unreadable` | Disk drift | Reinstall | The package record is present, but the install root needs to be recreated. |
| `missing_msi_file`, `msi_file_not_a_file`, `msi_file_unreadable`, `msi_file_permission_denied`, `msi_file_hash_mismatch`, `msi_file_hash_unavailable` | Disk drift | File restore | The recorded file content no longer matches the snapshot. |
| `missing_msi_inventory_snapshot`, `msi_inventory_unreadable`, `pkgdb_unreadable`, `incomplete_package_journal`, `unreadable_package_journal`, `malformed_package_journal`, `missing_journal_metadata` | Recovery trail missing | None | The recovery trail is incomplete, but doctor does not assign a direct repair action yet. |
| `orphan_install_directory` | Incomplete install | Orphan cleanup | A package directory exists without a matching database record. |
| `orphan_package_journal` | Incomplete install | Journal replay | A committed journal exists without a live package row. |
| `stale_package_journal`, `trailing_package_journal` | Conflict | Journal replay | The committed journal disagrees with the installed package and is treated as the recovery source. |

Diagnostics without a recovery mapping still appear in the health report, but they are report-only until a repair path is defined.

## 3. Repair Groups

Repair should be grouped by risk, not by individual line item.

### Low Risk: Journal Replay

- Use this when a committed journal can reconstruct missing SQLite state.
- Show all replay candidates as one batch.
- Ask once for the whole batch.
- Default answer is No.

### Medium Risk: Orphan Cleanup

- Use this for stale package records or recovery artifacts that can be removed without touching live content.
- Group these actions together when they belong to the same cleanup pass.
- Ask once for the whole batch.
- Default answer is No.

### High Risk: File Restore / Reinstall

- Use this when disk drift means the content itself is wrong.
- Ask separately for each package or file set.
- Do not collapse high-risk actions into a single bulk confirmation.
- Default answer is No.

### Destructive Operations

- Destructive actions are opt-in.
- If the operation removes data, the default is always No.
- The `-y` flag may pre-approve grouped low-risk actions, but it should not silently take destructive or high-risk actions.

Example confirmation flow:

- 3 problems found:
  - demo.exe is missing
  - registry state is inconsistent
  - journal is incomplete
- Then prompt by group, for example:
  - Replay 2 committed journals? [y/N]
  - Clean up 1 orphaned package record? [y/N]
  - Reinstall 1 package to fix disk drift? [y/N]

## 4. Journal Retention Policy

Committed journals are retained under `data/pkgdb` for the package lifetime unless we later introduce an explicit garbage-collection policy.

This means:

- The journal is part of the recovery trail, not the runtime state store.
- Missing journals reduce recoverability, but they do not prove that a package is broken.
- `recovery trail missing` is a Warning in the current policy.
- If we later make journal retention mandatory for all supported installs, the same diagnostic should be promoted to Error without changing the underlying classification.
- The per-package journal shape and package-type evidence details are defined in [Engine Roadmap and Ownership](engines.md).

Implementation consequence:

- `orphan_package_journal` stays Warning for now.
- If retention becomes a hard guarantee, the severity should be raised to Error in the doctor scan.

## 5. Operational Summary

- Committed journal wins over SQLite for rebuilds, but only for committed and structurally valid journals.
- Incomplete or malformed journals never override SQLite.
- Disk drift should be reported separately from state conflicts and should point the user toward reinstall or restore.
- Recovery tooling should prefer grouped confirmations for low-risk work and separate confirmations for high-risk work.

This policy is intentionally conservative. It keeps runtime state, recovery history, and file integrity in separate lanes so doctor can report clearly and repair can stay predictable.