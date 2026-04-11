# winbrew-windows

`winbrew-windows` is the Windows platform abstraction layer used by WinBrew.
It keeps Windows-specific implementation details behind a small root-level API,
so the rest of the workspace can depend on stable, easy-to-read entry points
instead of the Microsoft `windows` projection types directly.

## What this crate owns

This crate is responsible for the Windows-only boundary around three areas:

- filesystem inspection and extraction helpers
- registry enumeration for installed applications and uninstall roots
- MSIX deployment helpers for install and remove flows

The public API is intentionally exposed from `src/lib.rs` only. Internal module
layout is not part of the contract and can change without breaking consumers.

## Public surface at a glance

| Item | Purpose | Typical caller |
| --- | --- | --- |
| `inspect_path` | Inspect a path and return directory / reparse-point / hard-link metadata | archive extraction and cleanup code |
| `create_extracted_file` | Create a fresh file for extraction without following existing reparse points | archive extractors |
| `collect_installed_apps` | Enumerate installed applications from the uninstall registry roots | list / doctor commands |
| `uninstall_roots` | Iterate over the registry locations that may contain uninstall entries | registry browsing and diagnostics |
| `msix_install` | Install an MSIX package from a downloaded file and return the installed package full name | engine install flow |
| `msix_installed_package_full_name` | Resolve the installed full name for a package name or family name | MSIX receipt creation |
| `msix_remove` | Remove an installed MSIX package by full package name | engine remove flow |

## `src/lib.rs` root facade

`src/lib.rs` is intentionally small. It declares private modules and then
re-exports the stable API from the crate root:

```rust,ignore
#![cfg(windows)]
#![doc = include_str!("../README.md")]
#![allow(missing_docs)]

mod deployment;
mod fs;
mod registry;

pub use deployment::{msix_install, msix_installed_package_full_name, msix_remove};
pub use fs::{PathInfo, create_extracted_file, inspect_path};
pub use registry::{AppInfo, Hive, UninstallRoot, collect_installed_apps, uninstall_roots};
```

That shape matters for two reasons:

1. Consumers only import `winbrew_windows::...` and do not care about the
   internal folder tree.
2. The implementation can keep evolving while the root API stays predictable.

## Filesystem helpers

### `PathInfo`

`PathInfo` is a compact metadata snapshot returned by `inspect_path`.

- `is_directory` tells you whether the path is a directory.
- `is_reparse_point` tells you whether the path has the reparse-point flag.
- `hard_link_count` reports the number of hard links attached to the entry.

This is usually enough for cleanup and extraction logic. The struct intentionally
stays small so callers do not need to think about the lower-level Windows handle
APIs unless they want to.

### `inspect_path`

Use `inspect_path` when you need to decide what to do with an existing path
before writing or removing data.

Internally it opens the path with Windows handle APIs, reads the handle
information, and returns the three bits of state that WinBrew needs.

```rust,no_run
use std::path::Path;
use winbrew_windows::inspect_path;

let info = inspect_path(Path::new(r"C:\Temp\payload.msix")).unwrap();
println!("dir={} reparse={} links={}", info.is_directory, info.is_reparse_point, info.hard_link_count);
```

### `create_extracted_file`

Use `create_extracted_file` when you are creating a brand-new file that came
out of an archive or package and you want the filesystem operation to fail if
the target already exists.

It is a small helper around `OpenOptions` with the flags WinBrew expects for
fresh extraction targets.

```rust,no_run
use std::path::Path;
use winbrew_windows::create_extracted_file;

let _file = create_extracted_file(Path::new(r"C:\Temp\extract\tool.exe")).unwrap();
```

## Registry helpers

### `Hive`, `UninstallRoot`, and `uninstall_roots`

The uninstall registry data comes from three common locations:

- `HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall`
- `HKLM\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall`
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall`

`Hive` identifies whether a root is backed by `HKLM` or `HKCU`.
`UninstallRoot` is a snapshot of one discoverable registry branch together with
its label and key handle. `uninstall_roots()` returns only the roots that exist
on the current machine, so callers can iterate lazily without allocating a full
collection first.

```rust,no_run
use winbrew_windows::uninstall_roots;

for root in uninstall_roots() {
    println!("{} -> {}", root.hive, root.label);
}
```

### `AppInfo` and `collect_installed_apps`

`collect_installed_apps` walks the available uninstall roots, reads the
`DisplayName`, `DisplayVersion`, and `Publisher` values, and returns them as
`AppInfo` entries.

The `filter` argument is treated as a case-insensitive literal search. Any regex
metacharacters in the filter are escaped before matching, so the caller does not
need to think about regex syntax.

The result list is sorted by name first and then by version in descending
lexicographic order. After sorting, entries with the same name are deduplicated
so the first entry for each name wins. That keeps the highest version encountered
for each application name, which is good enough for display and removal workflows,
but it is not a semantic-version comparison.

```rust,no_run
use winbrew_windows::collect_installed_apps;

let apps = collect_installed_apps(Some("winbrew")).unwrap();

for app in apps {
    println!("{} {} - {}", app.name, app.version, app.publisher);
}
```

## MSIX deployment helpers

### `msix_install`

`msix_install` installs an MSIX package from a downloaded file path and returns
the installed package full name as a `String`.

The install flow canonicalizes the path, converts it into a file URI, and then
asks the Windows deployment APIs to install the package asynchronously. Once the
installation finishes, the helper resolves the installed package full name so
the caller can store it in a receipt.

```rust,no_run
use std::path::Path;
use winbrew_windows::msix_install;

let full_name = msix_install(Path::new(r"C:\Temp\packages\Contoso.App.msix"), "Contoso.App").unwrap();
println!("installed package: {}", full_name);
```

### `msix_installed_package_full_name`

Use this helper when you know a package name or family name but need the exact
installed full name.

The lookup returns one of three outcomes:

- exactly one match, which becomes the installed full name
- no matches, which returns an error
- multiple matches, which also returns an error so the caller can decide how to
  disambiguate

This is primarily a receipt helper for install flows.

### `msix_remove`

`msix_remove` removes an MSIX package by its exact full name. In other words,
it expects the value that was stored in the install receipt, not just a friendly
package display name.

```rust,no_run
use winbrew_windows::msix_remove;

msix_remove("Contoso.App_1.0.0.0_x64__8wekyb3d8bbwe!App").unwrap();
```

## Typical usage patterns

### 1. Install an MSIX package and keep the receipt

The engine layer uses this crate in two steps:

1. download the package file
2. call `msix_install`
3. store the returned full name in the engine receipt

That makes removal possible later without having to rediscover the package.

### 2. Remove a package

Removal is the inverse of install:

1. load the installed package metadata
2. extract the stored MSIX full name
3. call `msix_remove`

This avoids ambiguous package-name lookups during removal.

### 3. Inspect before extraction

Archive and portable extractors should check a path with `inspect_path`
before they overwrite or replace anything. That is the safest way to detect a
directory, a reparse point, or a path with unexpected hard-link behavior.

### 4. Discover installed software

`collect_installed_apps(None)` gives you a broad inventory. Passing
`Some("contoso")` narrows it down to matching display names without exposing
the caller to regex syntax.

## Non-Windows behavior

The crate is guarded by `#![cfg(windows)]`, so it only exists on Windows
targets. The public functions also return explicit errors when a non-Windows
build somehow reaches a platform-specific path.

That combination keeps the rest of the workspace honest: Windows behavior lives
here, and higher layers can stay portable.

## Maintenance notes

- Keep the root API small and stable.
- Add new Windows-specific functionality behind private modules first, then
  re-export only the minimal public surface.
- Prefer returning plain Rust types from this crate so the rest of the workspace
  does not depend on Windows projection types.
- When a helper needs to expose more behavior, document the caller-facing
  contract at the root and keep the low-level implementation details private.