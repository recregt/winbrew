# MSI Inventory Scanner

`winbrew_windows::deployment::msi` is the Windows-only inventory scanner that
turns an MSI database into the `MsiInventorySnapshot` shape persisted by
WinBrew storage.

For the crate-level overview of the Windows facade, see [the `winbrew-windows` README](../../../README.md).

The module is intentionally read-only. It does not install, remove, repair, or
mutate MSI data. Its only job is to extract structure from an MSI database,
resolve the parts that can be resolved safely, and return a conservative
snapshot that downstream code can persist in SQLite.

## Why this module exists

WinBrew stores MSI package state as normalized records in the database layer.
The canonical MSI source, however, is the package database itself. That means
the Windows integration has to bridge two worlds:

- the MSI schema, which is table-driven and relatively low level
- the WinBrew storage model, which expects typed, normalized records

This module is the bridge.

It reads the standard MSI tables, reconstructs directory and file paths using
the package's install root, converts registry and shortcut rows into normalized
records, and attaches package identity metadata from the `Property` table.

## Public entry point

### `scan_inventory`

`scan_inventory(package_path, install_root, package_name, scope)` is the only
public function in the module. It performs the entire scan and returns an
`MsiInventorySnapshot`.

Inputs:

- `package_path`: path to the MSI database file
- `install_root`: root directory used to resolve directory and file paths
- `package_name`: WinBrew package identity used in the resulting records
- `scope`: install scope used for registry hive interpretation

Output:

- `receipt`: package identity, product code, upgrade code, and scope
- `files`: file records with normalized paths
- `registry_entries`: registry entries with normalized hive and key paths
- `shortcuts`: shortcut records with normalized shortcut and target paths
- `components`: component records with normalized key-path locations

The function is intentionally conservative. If the module cannot prove a path,
it prefers a missing path or a raw value over inventing a likely-but-wrong one.

## High-level data flow

The scanner runs in a fixed order:

1. Open the MSI database read-only.
2. Query package identity from the `Property` table.
3. Load the core MSI tables into internal row structs.
4. Resolve the `Directory` tree into concrete paths.
5. Build a file path map once and reuse it across builders.
6. Convert rows into WinBrew model records.
7. Return the completed snapshot.

That order matters. The directory tree must be resolved before file, shortcut,
and component builders can normalize their output. The precomputed file path map
keeps all downstream records aligned to the same derived path.

## Module layout

The scanner is split into four internal modules so each responsibility stays
isolated and testable.

### `database.rs`

Owns the MSI handle layer and all raw database access.

Responsibilities:

- open the MSI database with `MsiOpenDatabaseW`
- wrap MSI handles in a small RAII type so they are always closed
- execute parameterless queries
- fetch rows from MSI views
- decode MSI strings strictly as UTF-16
- collect table rows into internal row structs

Important behavior:

- string extraction uses a two-pass read so the buffer is sized by the MSI API
- `ERROR_MORE_DATA` is treated as the normal probe path, not as a failure
- invalid UTF-16 is surfaced as an error instead of being lossy-decoded
- every MSI API failure is wrapped with context so callers know which step
  failed

This module should be the only place that knows about the Windows Installer C
API details.

### `directory.rs`

Owns directory-tree resolution.

Responsibilities:

- resolve the `Directory` table into a `HashMap<String, PathBuf>`
- recursively follow parent links
- detect cycles in the MSI directory graph
- treat `TARGETDIR` and `SOURCEDIR` as roots anchored at `install_root`

The directory tree is resolved with a memoized depth-first walk. The module
tracks the current recursion stack in a `visiting` set so it can fail on cycles
instead of recursing forever.

If a row is missing, the module returns a contextual error. It does not guess a
directory path that the MSI database did not define.

### `builder.rs`

Owns the conversion from resolved rows into WinBrew model records.

Responsibilities:

- build file paths and file records
- build registry records with scope-aware hive mapping
- build shortcut records
- build component records

This module is where the row data becomes the schema that storage expects.

The file builder intentionally uses a precomputed path map as the primary source
for file paths. That avoids recomputing the same derived path in multiple code
paths and keeps file, shortcut, and component records aligned to the same
normalization rules.

Registry mapping is scope-aware for `Root = -1`:

- `InstallScope::Installed` maps to `HKLM`
- `InstallScope::Provisioned` maps to `HKCU`

In the current code, `Provisioned` is the variant used for the non-installed
scope case that needs `HKCU` rather than `HKLM`. The enum itself only names the
scope; it does not add a stronger per-user guarantee beyond this mapping.

### `path.rs`

Owns path and reference normalization.

Responsibilities:

- normalize filesystem paths for storage keys
- normalize registry key paths
- select the correct MSI name form from `long|short` encoded values
- resolve MSI-style references into concrete paths when possible

This module contains pure logic and is the easiest place to unit test in
isolation.

## Internal row model

The scanner does not keep raw MSI records around. It translates the MSI tables
into small row structs first, then builds the final model from those structs.

| Row type | Source table | Purpose |
| --- | --- | --- |
| `DirectoryRow` | `Directory` | parent/child path resolution |
| `ComponentRow` | `Component` | component key-path lookup |
| `FileRow` | `File` | file path and file record generation |
| `RegistryRow` | `Registry` | registry record generation |
| `ShortcutRow` | `Shortcut` | shortcut record generation |

The `Property` table is queried separately for `ProductCode` and `UpgradeCode`
because those values are package identity, not inventory rows.

## Path resolution rules

### Directory paths

Directory paths are resolved by walking the `Directory` table hierarchy.

Rules:

- if a row has a parent, the parent is resolved first
- if the directory id is `TARGETDIR` or `SOURCEDIR`, the base is `install_root`
- if a row has no parent and is not a special root, the base still falls back to
  `install_root`
- the `DefaultDir` column is interpreted with MSI name selection rules

The module does not try to infer missing structure. If the MSI database omits a
required row or forms a cycle, the scan fails with a contextual error.

### File paths

File paths are resolved from the owning component's directory and the file row's
`FileName` field.

Rules:

- the component determines the directory anchor
- the directory anchor comes from the directory map when available
- the file name is normalized with `select_msi_name`
- the resulting path is joined onto the base directory

The precomputed `file_paths` map is used as the canonical source for derived
file paths when generating records. If a key is missing from the map, the code
falls back to direct row resolution.

### Shortcut and component references

Shortcut targets and component key paths can refer to MSI symbolic paths rather
than literal filesystem paths. `resolve_reference_path` handles the common forms:

- `[#FileKey]` for file references
- `[DirectoryId]suffix` for directory-relative references
- direct table keys when the value matches a file or directory id exactly
- literal filesystem paths when the value already looks like a path

If a reference cannot be resolved safely, the function returns `None` instead
of guessing.

### Name selection

MSI name fields often use `long|short` encoding.

`select_msi_name` follows this rule:

- prefer the long name when present and non-empty
- fall back to the short name when the long name is missing or `.`
- reject empty values and bare `.` markers

This behavior matters for directory names, file names, and shortcut names.

## Normalization rules

### Filesystem paths

`normalize_path` prepares a path for storage and comparison.

It performs three transformations:

- strips verbatim prefixes such as `\\?\` and `\\?\UNC\`
- replaces backslashes with forward slashes
- lowercases the final string

This is for identity and lookup, not for changing the actual filesystem path on
disk.

### Registry key paths

`normalize_registry_key_path` trims whitespace and lowercases the key.

That gives storage a stable comparison key even if the MSI author used mixed
case or trailing spaces.

## Error model

The module uses `anyhow::Result` everywhere so every failure can carry context.

Common failure classes:

- Windows API failures when opening the database or fetching rows
- missing required package metadata such as `ProductCode`
- invalid UTF-16 returned by MSI string fields
- directory graph cycles
- missing directory rows required to resolve the tree

The scanner is read-only, so there is no rollback or partial write behavior here.
If it fails, nothing has been persisted yet.

## Relationship to database

The scanner does not write to SQLite. It only produces the snapshot shape that
`winbrew-database` knows how to persist.

That separation is deliberate:

- `msi` reads and normalizes MSI data
- `database` owns transactional persistence and reverse lookups
- `app` decides when a successful scan should be committed

Keeping those responsibilities separate makes the MSI scan logic easier to
test, easier to reason about, and easier to reuse for doctor or recovery flows.

## Testing strategy

The module is covered by focused unit tests:

- `path.rs` validates name selection and normalization behavior
- `builder.rs` validates scope-aware registry root mapping

The database and directory modules are mostly covered indirectly through the
scanner and through downstream integration tests. More exhaustive MSI fixture
tests can be added later if a stable sample database is introduced.

To run the relevant checks:

```bash
cargo test -p winbrew-windows
cargo test -p winbrew-app
```

## Extension points

If the scanner needs to grow, keep new responsibilities in the same split:

- add raw table access in `database.rs`
- add graph or tree resolution in `directory.rs`
- add record shaping in `builder.rs`
- add pure normalization logic in `path.rs`

That keeps the module easy to navigate and avoids turning `mod.rs` back into a
monolith.

## Summary

This module is the Windows MSI intake boundary for WinBrew. It reads MSI
metadata, resolves only the paths that can be resolved safely, and produces the
normalized snapshot used by the rest of the application.

It is deliberately narrow, conservative, and testable.