# WinBrew Update Worker

This package hosts the D1-backed update gateway used by the CLI.

## What It Serves

- `GET /v1/update` is the only supported runtime route.
- Requests with `current=` can return `current`, `full`, or `patch` plans.
- Any other path returns `404`.

## Production Endpoint

The production route is configured on `api.winbrew.dev/v1/update` in [wrangler.jsonc](wrangler.jsonc). Wrangler still prints the `workers.dev` hostname after deploy, but Cloudflare routes live traffic for the configured route to this worker.

## Local Development

Run the worker from this directory:

```powershell
pnpm dev
```

`pnpm dev` bootstraps the local D1 database before Wrangler starts:

1. applies the checked-in schema from `migrations/0001_init.sql`
2. seeds a latest full plan from `seed/local-dev.sql`

Local bootstrap values live in [.dev.vars](.dev.vars). `WINBREW_UPDATE_DB_NAME` controls the local D1 name, while `WINBREW_UPDATE_DB_ID` and `WINBREW_UPDATE_PREVIEW_DB_ID` are optional and can be left empty to let Wrangler auto-provision the local database.

That keeps `wrangler dev` usable without any manual D1 setup.

## GitHub Actions

Pushes to `main` that change files under `infra/api/update/` deploy this worker automatically through [.github/workflows/update-api.yml](../../../.github/workflows/update-api.yml).

The workflow runs `pnpm test -- --run` before `pnpm exec wrangler deploy -c ./wrangler.jsonc` and expects these secrets in GitHub:

- `CLOUDFLARE_API_TOKEN`
- `CLOUDFLARE_ACCOUNT_ID`

## Tests

Run the package test suite with:

```powershell
pnpm test -- --run
```
