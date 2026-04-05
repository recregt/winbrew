[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

Push-Location $RepoRoot
try {
    Push-Location infra
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
} finally {
    Pop-Location
}