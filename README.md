# WinBrew

![Rust](https://img.shields.io/badge/Rust-000000?style=flat&logo=rust&logoColor=white)
![Go](https://img.shields.io/badge/Go-00ADD8?style=flat&logo=go&logoColor=white)
![Windows](https://img.shields.io/badge/Windows-0078D6?style=flat&logo=windows&logoColor=white)
![Cloudflare](https://img.shields.io/badge/Cloudflare-F38020?style=flat&logo=Cloudflare&logoColor=white)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE-MIT)
[![License: Apache 2.0](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](LICENSE-APACHE)

WinBrew is a catalog-first package manager for Windows. It resolves packages from a local SQLite catalog, delegates installs to Windows-native or filesystem engines, and keeps state under a managed root.

> [!IMPORTANT]
> This project is still under active development. Public end-user releases are not published yet, and there is no supported installer flow.
>
> For the architecture map, start with [docs/index.md](docs/index.md).

## Frequently Asked Questions

### What is WinBrew?
WinBrew is a Windows package manager that tries to make package search, installation, tracking, and cleanup deterministic. It does not query upstream package sites at runtime for every operation; instead, it works from a locally stored catalog snapshot.

That makes WinBrew closer to a managed package system than a thin wrapper around an existing package manager.

### Why should I use WinBrew instead of Winget or Scoop?
If one ecosystem already covers your needs, Winget or Scoop may be enough. WinBrew is aimed at people who want a single local catalog that can unify multiple sources, work offline once synced, keep install state in one managed root, and track recovery data per package.

The trade-off is that WinBrew is opinionated about its catalog model. In return, searches and installs are more deterministic because they are based on the same local database.

### How up to date are the packages?
WinBrew is snapshot-based, not live-query-based. The current catalog is whatever the latest successfully published snapshot contains, and the update pipeline can deliver either a full snapshot or a patch chain depending on the release plan.

That means freshness depends on the upstream sources and on the last successful publish, not on a real-time lookup against Winget or GitHub.

### If the catalog is rebuilt from scratch every night, how do I access older package versions?
It is not rebuilt from scratch every night. The pipeline publishes catalog artifacts on a schedule, and historical versions are reachable through archived catalog snapshots and retained release lineage when those artifacts are kept.

If a version was once published, the right place to look is the historical catalog artifact for that publish date, not a live upstream search. WinBrew does not yet ship a package-time-travel browser in the CLI.

### Why do you use Go and Rust together in a monorepo? Isn't that complex?
It is more complex than using one language everywhere, but the split is deliberate. The crawler and publisher are Go-based, while the parser, CLI, core library, database layer, models, and engines are Rust-based.

The monorepo keeps the contract between producer and consumer in one place. That reduces drift in the catalog schema, update flow, and CI wiring, which would be harder to control if the pieces lived in separate repositories.

### What does "Unified Catalog" mean if Winget and Scoop package IDs are different?
It means the catalog normalizes multiple sources into one model without pretending their upstream IDs are the same. WinBrew keeps the source as part of the identity: Winget stays `winget/<id>`, Scoop stays `scoop/<bucket>/<id>`, Chocolatey stays `chocolatey/<id>`, and WinBrew stays `winbrew/<id>`.

Search is still unified because name-based queries can match across sources. If more than one package matches, the CLI asks you to choose.

### Does WinBrew host the package payloads itself?
No. WinBrew hosts the catalog bundle and the update metadata, not the actual installer payloads.

The installers still live on the upstream hosts that published them. WinBrew stores the metadata needed to find, verify, and install them.

### Isn't a 35 MB database too large?
Not for what it does. The database is not just package names; it contains normalized package metadata, installer details, search data, and update information.

SQLite buys you indexed queries, schema enforcement, atomic updates, and a clean way to publish stable snapshots. The release pipeline also uses zstd compression, and the release archive can carry both the raw `catalog.db` and the compressed `catalog.db.zst` so you can choose whichever form you want.

For an offline-first catalog, that is usually a better trade than JSON, YAML, or XML.

### Why use SQLite instead of JSON, YAML, or XML?
Because the catalog is an operational database, not a config file. WinBrew needs fast local search, indexed filtering, atomic refreshes, schema versioning, and patch-friendly update behavior.

JSON, YAML, and XML are great interchange formats, but they are a poor fit for a large runtime catalog that must be queried and updated safely under transaction semantics.

### Can I search without internet access?
Yes, as long as the catalog is already present. `winbrew search` queries the local `catalog.db`, so internet access is only needed to fetch or refresh the catalog.

If the catalog is missing, WinBrew will tell you to run `winbrew update` first.

### Does WinBrew track my searches or do telemetry?
No, the network traffic WinBrew does perform is for catalog updates, installer downloads, and the update API. That is different from telemetry.

### Why is there no pre-built CLI binary yet? Only the catalog is ready!
Because the project is still changing and the supported flow is source-first. Releases are not published yet, and the repository still expects you to build the CLI locally while the catalog/update contracts continue to evolve.

The local CLI binary exists as `winbrew-bin`, but the repo does not currently ship a public end-user installer or pre-built release artifact.

### Can I use WinBrew from a USB drive or portable location?
Only partially. The CLI can run from wherever you place the binary, and the managed root can be redirected with `WINBREW_PATHS_ROOT`, which helps for portable-style setups.

That said, WinBrew is not designed as a fully portable app suite. Some install engines are machine-bound, and installed packages still depend on the target Windows system.

### Why only Windows? Will you add Linux or macOS package support later?
The current engine layer is built around Windows-specific behavior: Windows Installer, App Installer, registry-aware cleanup, fonts, path conventions, and Windows-only helpers. Linux and macOS are not in scope today.

Adding APT or Homebrew support would be a separate product-level effort, not a small feature toggle. There is no equivalent cross-platform roadmap in the current repository.

### Can I connect my company's private package repository?
Not as a generic plug-in feed today. The current ingestion pipeline is built around Winget and Scoop, and the runtime update flow points at WinBrew's own update gateway.

If you need private packages, the current path is to extend the pipeline or mirror those packages into a supported source model. It is not a config-only feature yet.

### Can I install paid or licensed software with WinBrew?
Yes, if the package is present in the supported source model and you are allowed to install it. The catalog model already handles proprietary software metadata.

WinBrew does not bypass license checks, payment gates, or vendor terms. It only automates discovery, verification, and installation of packages you are entitled to use.

### Is WinBrew safe?
It is designed to reduce risk, not to magically make third-party software trustworthy. WinBrew validates the catalog bundle hash, verifies installer hashes, and rejects legacy checksum algorithms by default.

That said, the trust model still depends on upstream package sources and the publish pipeline. If a source is malicious or compromised, no package manager can eliminate that risk entirely.

### How does the actual installation work?
WinBrew first resolves a package against the catalog, then selects the best installer, downloads the payload, verifies it, and hands it to the right engine.

For MSI, MSIX, native EXE, and font packages, the final install step is delegated to Windows-native mechanisms. For ZIP and portable packages, WinBrew handles the file-system work itself.

### If the same package exists in both Winget and Scoop, will there be a conflict?
Not at the catalog level. WinBrew keeps the source as part of the identity, so the Winget and Scoop entries remain separate records even if they share the same visible name.

If you search by name and more than one record matches, the CLI asks you to choose which one you meant. If you want to be explicit, use the source-tagged ID.

### Will WinBrew break my existing Winget or Scoop installation?
No. WinBrew keeps its own managed root and does not rewrite Winget or Scoop's databases, folders, or settings.

You can still run into application-level overlap if multiple package managers install the same software, but that is not the same as WinBrew damaging your existing Winget or Scoop installation.

### How do I know nobody slipped a malicious or fake package link into the catalog?
You cannot get an absolute guarantee from any package manager that relies on third-party sources. WinBrew reduces the attack surface by publishing controlled catalog bundles, verifying metadata hashes, and verifying installer hashes before installation.

That makes accidental corruption and casual tampering harder. It does not make the upstream ecosystem magically safe if a source is intentionally compromised.

### Is WinBrew a background service that keeps running and eating RAM?
No. The client is not a persistent daemon or Windows service. It runs when you invoke a command and exits when the command is done.

There is a separate update worker in the infrastructure layer, but that is not a process that runs on your machine all the time.

### If I stop using WinBrew or delete it, what happens to the packages I installed? Is there vendor lock-in?
Installed packages remain installed unless you remove them. WinBrew keeps its own metadata in SQLite and per-package journal files, but deleting WinBrew does not automatically uninstall the applications it previously managed.

That means the lock-in is low. You can cleanly remove packages with WinBrew before uninstalling WinBrew itself, or you can leave the packages in place and manage them through the underlying Windows mechanisms.

### What happens if an install is interrupted or the catalog state is inconsistent?
WinBrew writes per-package recovery journals under `data/pkgdb/<package-key>/journal.jsonl`. If a package install is interrupted, a committed journal can be replayed to rebuild package state, SQLite remains the normal runtime index, and disk is the source of truth for file content checks.

`winbrew doctor` classifies missing SQLite, missing journals, conflicts, and disk drift. `winbrew repair` replays committed journals first and then handles cleanup or higher-risk fixes with confirmation when needed.

Committed journals are retained for the package lifetime, so recovery information survives partial installs and process crashes.

### Do I need Administrator rights or UAC for search or installation?
Search, update checks, configuration, and other read-only operations do not require elevated rights. They work from your user context and the local catalog.

Installation and removal depend on the package and the engine. User-scoped or portable installs often do not need UAC, while machine-wide MSI/MSIX or system-integrated installs may prompt Windows for elevation.

### Will it work behind a strict corporate proxy or firewall?
Only if the network can reach the endpoints WinBrew needs. The client talks to the update API, CDN-hosted catalog artifacts, and the package hosts that actually serve installers.

If those destinations are blocked, updates and installs will fail. If the catalog is already present, offline search can still work.

### Does WinBrew use a single point of failure somewhere?
For local search and install, the client is not a single point of failure once the catalog is already present. For catalog refresh, the update gateway, CDN, D1 materialization, and publisher pipeline are central dependencies by design.

That is an intentional trade-off: the local client is resilient once synced, but catalog refresh depends on the update plane being available.

### The developer could leave tomorrow. Would the system break?
No. The client is not a permanent service and WinBrew is not embedded into the Windows boot path.

If development stopped, the catalog and update experience would eventually stall, but the machine itself would not stop working and the software you already installed would keep running.

### I do not code. How can I contribute?
You can still help a lot. Good non-code contributions include writing reproducible bug reports, testing on different Windows versions, validating catalog behavior, improving documentation, checking package metadata, and filing issues with clear logs and steps.

If you want a low-friction way to help, start with docs review and issue reproduction. If you later want to move closer to the codebase, the best entry points are usually tests, fixtures, and documentation corrections.
