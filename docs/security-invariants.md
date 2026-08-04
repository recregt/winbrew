# Security Invariants

This page collects two rules that recur across the codebase wherever it
crosses a trust boundary: the network and the filesystem. Both are already
enforced everywhere they apply today. This page exists so a *new* ingestion
point defaults to the same pattern instead of re-deriving it.

## 1. Verify before you act, never after

Every flow that takes a network-delivered payload and turns it into a
filesystem write or a database mutation hash-verifies that payload **before**
it is written or executed — not after.

Current examples:

- `crates/app/src/operations/install/download.rs` verifies the installer hash
  before the engine ever sees the downloaded file.
- `crates/app/src/operations/install/sevenz.rs` verifies the 7-Zip runtime
  bootstrap the same way.
- `crates/app/src/operations/update/patch.rs` hash-verifies each catalog patch
  before `execute_batch` runs it, and the extracted content is bounded by a
  restrictive SQLite authorizer (see the module doc comment there for the
  full ordering).
- `crates/core/src/fs/archive/extract/sevenz.rs` runs `verify_extracted_tree`
  against the same symlink/depth/size limits the zip/tar backends enforce,
  after 7z.exe extracts, since 7z.exe itself can't be handed the validation
  callback mid-extraction.

If you add a new place that downloads, decompresses, or otherwise ingests
data from `api.winbrew.dev`, a CDN, or any other network source: verify a
hash or signature obtained out-of-band before that data is written to disk,
inserted into SQLite, or executed — no exceptions, and no "verify after, roll
back on failure" shortcuts. A rollback after partial execution is not the
same guarantee as never executing untrusted input in the first place.

## 2. Path-safety belongs at the model boundary, not the call site

A string that becomes part of a filesystem path — a package name, a command
name, a manifest field used to build a cache path — is validated once, where
the type is constructed, not re-validated at each of its N consumers.

Current examples:

- Rust: `ensure_safe_path_component` (`crates/models/src/shared/validation.rs`)
  runs on `CatalogPackage.name` and `PackageRef` at construction. Every
  consumer downstream — `package_install_dir`, command shim paths, journal
  replay — inherits the guarantee instead of re-checking it.
- Go: `validateManifestPathSegment` (`infra/crawler/pkg/sources/winget/manifest.go`)
  and `validateRepoInputs` (`infra/crawler/pkg/sources/scoop/git.go`) apply
  the same rule at the point community-editable manifest/bucket data is
  turned into a local path or a git remote.

Go and Rust don't share code for this (there's no shared runtime between the
two languages), but they share the invariant: reject `..`, path separators,
and empty/control-character values before the string is allowed to become
part of a path, at the boundary where the value enters the system — not
downstream. If you add a new field sourced from the catalog, a manifest, or
any other external input that will eventually feed a `Path::join` /
`filepath.Join`, validate it there, not at the `remove_dir_all` call site
three layers away.
