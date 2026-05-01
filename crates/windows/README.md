 # winbrew-windows

`winbrew-windows` is the Windows platform abstraction layer used by WinBrew.
It keeps Windows-specific implementation details behind a small root-level API,
so the rest of the workspace can depend on stable, easy-to-read entry points
instead of the Microsoft `windows` projection types directly.

Consumers import the facade modules `installed`, `host`, `fonts`, `fs`, and
`packages` instead of reaching into the internal folder tree. Test helpers live
behind the `testing` facade and are only available to unit tests or when the
`testing` feature is enabled.

## What this crate owns

This crate is responsible for the Windows-only boundary around five areas:

- filesystem inspection and extraction helpers
- registry enumeration for installed applications, uninstall roots, and named uninstall values
- per-user font registration and session loading helpers
- MSIX deployment helpers for install and remove flows
- MSI inventory scanning for package databases and install trees

The public API is intentionally exposed from `src/lib.rs` only. Internal module
layout is not part of the contract and can change without breaking consumers.

## Public surface at a glance

| Item | Purpose | Typical caller |
| --- | --- | --- |
| `installed::*` | Installed applications and uninstall registry values | list / doctor commands |
| `host::*` | Host profile, elevation, PATH, and Windows version helpers | installer selection and info output |
| `fonts::*` | Per-user font install and removal helpers | font engine |
| `packages::*` | MSI / MSIX package helpers | engine install / remove flow |
| `fs::*` | Filesystem inspection and extraction helpers | archive extraction and cleanup code |
| `testing::*` | Test-only registry helpers | unit tests and test binaries |

## `src/lib.rs` root facade

`src/lib.rs` is intentionally small. It declares private modules and then
exposes the stable API through facade modules instead of flat root re-exports:

```rust,ignore
#![cfg(windows)]
#![doc = include_str!("../README.md")]

mod deployment;
mod font;
#[path = "fs/mod.rs"]
mod filesystem;
mod registry;
mod system;

pub(crate) use winbrew_models as models;

pub mod installed {
  pub use crate::registry::{
    AppInfo, UninstallEntry, installed_apps, installed_apps_matching,
    read_uninstall_registry_value, uninstall_entries, uninstall_entries_matching,
  };
}

pub mod host {
  pub use crate::system::{
    HostProfile, host_profile, is_elevated, search_path_file, windows_version_string,
  };
}

pub mod fonts {
  pub use crate::font::{install_user_font, remove_user_font, user_fonts_dir};
}

pub mod packages {
  pub use crate::deployment::{
    msi_scan_inventory, msix_install, msix_installed_package_full_name, msix_remove,
  };
}

pub mod fs {
  pub use crate::filesystem::{PathInfo, create_extraction_target_file, inspect_path};
}

#[cfg(any(test, feature = "testing"))]
pub mod testing {
  pub use crate::system::windows_version_string;
  pub use crate::registry::{
    UninstallEntryGuard, create_test_uninstall_entry,
    create_test_uninstall_entry_with_install_location,
  };
}
```

## System helpers

### `HostProfile`

`HostProfile` combines the current Windows host family and native architecture
into one snapshot. Call `host_profile()` once, then inspect `is_server` and
`architecture` when you need to branch. The snapshot also exposes
`platform_tags()` so installer selection can map the host family to the catalog
labels WinBrew accepts. Normal hosts currently accept `windows.desktop`,
`windows.ltsc`, and `windows.universal`; server hosts accept `windows.server`.

Use `host::is_elevated()` when you need to decide whether machine-scope
installers should be preferred over user-scope installers.

```rust,no_run
use winbrew_windows::host::host_profile;

let profile = host_profile();
println!("host: {profile}");
println!("server: {}", profile.is_server);
println!("architecture: {}", profile.architecture);
```

If Windows cannot read the product-type registry value, the helper falls back
to a normal client host. Unknown processor architecture codes still map to
`Architecture::Any`.

### `windows_version_string`

`windows_version_string` returns the current Windows version string when the
registry exposes the required values. It prefers the numeric
`CurrentMajorVersionNumber` and `CurrentMinorVersionNumber` values, falls back
to `CurrentVersion` when needed, and appends `CurrentBuildNumber` plus `UBR`
when they are available.

Use this helper for version banners or `info`-style output when you want the
version line to stay registry-backed but still return a plain string.

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

## Font helpers

### `user_fonts_dir`

`user_fonts_dir` returns the per-user font directory WinBrew uses for Windows
font installation: `%LOCALAPPDATA%\Microsoft\Windows\Fonts`.

### `install_user_font`

`install_user_font` copies a supported font file into the user font directory,
writes the HKCU registration entry under
`Software\Microsoft\Windows NT\CurrentVersion\Fonts`, and loads the font into
the current session with `AddFontResourceExW`.

The helper accepts raw `.ttf`, `.otf`, `.ttc`, and `.otc` payloads.

### `remove_user_font`

`remove_user_font` removes the session registration and deletes the copied font
file from the per-user directory. Unload is best-effort so the helper remains
idempotent if Windows has already dropped the session resource.

```rust,no_run
use std::path::Path;
use winbrew_windows::fonts::{install_user_font, remove_user_font, user_fonts_dir};

let installed = install_user_font(Path::new(r"C:\Temp\fixture.ttf")).unwrap();
println!("installed font: {}", installed.display());

let fonts_dir = user_fonts_dir().unwrap();
println!("user fonts dir: {}", fonts_dir.display());

remove_user_font(&installed).unwrap();
```

### `inspect_path`

Use `inspect_path` when you need to decide what to do with an existing path
before writing or removing data.

Internally it opens the path with Windows handle APIs, reads the handle
information, and returns the three bits of state that WinBrew needs.

```rust,no_run
use std::path::Path;
use winbrew_windows::fs::inspect_path;

let info = inspect_path(Path::new(r"C:\Temp\payload.msix")).unwrap();
println!("dir={} reparse={} links={}", info.is_directory, info.is_reparse_point, info.hard_link_count);
```

### `create_extraction_target_file`

Use `create_extraction_target_file` when you are creating a brand-new file that came
out of an archive or package and you want the filesystem operation to fail if
the target already exists.

It is a small helper around `OpenOptions` with the flags WinBrew expects for
fresh extraction targets.

```rust,no_run
use std::path::Path;
use winbrew_windows::fs::create_extraction_target_file;

let _file = create_extraction_target_file(Path::new(r"C:\Temp\extract\tool.exe")).unwrap();
```

## Installed app helpers

### `UninstallEntry` and `uninstall_entries`

The uninstall registry data comes from three common locations:

- `HKLM\Software\Microsoft\Windows\CurrentVersion\Uninstall`
- `HKLM\Software\WOW6432Node\Microsoft\Windows\CurrentVersion\Uninstall`
- `HKCU\Software\Microsoft\Windows\CurrentVersion\Uninstall`

`uninstall_entries()` returns a vector of registry snapshots for all available
entries. Use `uninstall_entries_matching()` when you want a case-insensitive
literal display-name filter. Each `UninstallEntry` contains plain Rust strings
for the commonly used uninstall fields, so callers do not need to work with
registry handles or root snapshots.

```rust,no_run
use winbrew_windows::installed::uninstall_entries_matching;

for entry in uninstall_entries_matching("winbrew").unwrap() {
  println!("{} {}", entry.display_name, entry.version);
}
```

### `AppInfo` and `installed_apps`

`installed_apps()` walks the available uninstall roots, reads the `DisplayName`,
`DisplayVersion`, and `Publisher` values, and returns them as `AppInfo`
entries. Use `installed_apps_matching()` when you want to filter by a literal
display-name match.

The result list is sorted by name first and then by version in descending
lexicographic order. After sorting, entries with the same name are deduplicated
so the first entry for each name wins. That keeps the highest version encountered
for each application name, which is good enough for display and removal workflows,
but it is not a semantic-version comparison.

```rust,no_run
use winbrew_windows::installed::installed_apps_matching;

let apps = installed_apps_matching("winbrew").unwrap();

for app in apps {
    println!("{} {} - {}", app.name, app.version, app.publisher);
}
```

### `read_uninstall_registry_value`

`read_uninstall_registry_value` searches the uninstall roots for a key name and
then reads the first non-empty string value with the requested value name. MSI
install flows use it to read `InstallLocation` immediately after `msiexec`
completes, so the engine can record the final path Windows reports instead of
assuming the requested install directory is always the truth.

```rust,no_run
use winbrew_windows::installed::read_uninstall_registry_value;

let install_location = read_uninstall_registry_value(
  "{11111111-1111-1111-1111-111111111111}",
  "InstallLocation",
);
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
use winbrew_windows::packages::msix_install;

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
use winbrew_windows::packages::msix_remove;

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

## MSI inventory scanner

### `msi_scan_inventory`

`msi_scan_inventory` opens an MSI database in read-only mode, walks the standard
inventory tables, and reconstructs the snapshot shape that WinBrew stores in
SQLite.

For the module-level design, responsibilities, and path-resolution rules, see
[the detailed MSI scanner README](src/deployment/msi/README.md).

The helper expects three inputs:

- the MSI database file path
- the install root used to resolve directory and file paths
- the package name and install scope that WinBrew should persist

It is intentionally best-effort on path resolution. Directory trees, file keys,
and shortcut targets are resolved when the MSI tables provide enough structure;
otherwise the scanner keeps the raw database data conservative instead of
inventing a path that might be wrong.

```rust,no_run
use std::path::Path;
use winbrew_models::domains::install::InstallScope;
use winbrew_windows::packages::msi_scan_inventory;

let snapshot = msi_scan_inventory(
  Path::new(r"C:\Temp\packages\Contoso.App.msi"),
  Path::new(r"C:\Program Files\WinBrew\packages\Contoso.App"),
  "Contoso.App",
  InstallScope::Installed,
)
.unwrap();
```

### 3. Inspect before extraction

Archive and portable extractors should check a path with `inspect_path`
before they overwrite or replace anything. That is the safest way to detect a
directory, a reparse point, or a path with unexpected hard-link behavior.

### 4. Discover installed software

`installed_apps()` gives you a broad inventory. Passing `installed_apps_matching("contoso")`
narrows it down to matching display names without exposing the caller to regex
syntax.

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