# WinBrew Update Worker

This package hosts the D1-backed update gateway used by the CLI.

## What It Serves

- `GET /v1/update` is the only supported runtime route.
- Requests with `current=` can return `current`, `full`, or `patch` plans.
- Any other path returns `404`.

## Local Development

Run the worker from this directory:

```powershell
pnpm dev
```

`pnpm dev` bootstraps the local D1 database before Wrangler starts:

1. applies the checked-in schema from `migrations/0001_init.sql`
2. seeds a latest full plan from `seed/local-dev.sql`

That keeps `wrangler dev` usable without any manual D1 setup.

## Tests

Run the package test suite with:

```powershell
pnpm test -- --run
```
