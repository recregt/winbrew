[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

Push-Location $RepoRoot
try {
    foreach ($module in @('infra\crawler', 'infra\publisher')) {
        Push-Location $module
        try {
            go vet ./...
            if ($LASTEXITCODE -ne 0) {
                exit $LASTEXITCODE
            }

            go test ./...
            if ($LASTEXITCODE -ne 0) {
                exit $LASTEXITCODE
            }
        } finally {
            Pop-Location
        }
    }

    cargo test --workspace
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
} finally {
    Pop-Location
}