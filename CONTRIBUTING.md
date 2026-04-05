# Contributing

`WinBrew` uses **[go-task](https://taskfile.dev/)** and **[Lefthook](https://lefthook.dev/)** to manage the development workflow.

## Setup

```powershell
task tools:install-lefthook
task tools:install-nextest
lefthook install
```

## Common Tasks

| Command | Description |
| :--- | :--- |
| `task test` | Run Rust tests |
| `task test:nextest` | Run Rust tests with nextest |
| `task test:infra` | Run infra (Go) tests |
| `task ci:smoke` | Build and smoke-test the CLI |
| `task dev:run -- <args>` | Run locally without polluting your profile |
| `task dev:run-release -- <args>` | Run in release mode |
| `task dev:clean` | Clean the dev root |

`task dev:run` and `task dev:run-release` use `target\winbrew-dev` via `WINBREW_PATHS_ROOT`, so config, logs, and databases stay inside the repo.

You can pass any WinBrew arguments after `--`, for example `task dev:run -- doctor` or `task dev:run-release -- install firefox`.