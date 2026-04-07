[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

Push-Location $RepoRoot
try {
    Write-Host 'Running cargo fmt'
    $global:LASTEXITCODE = 0
    & cargo fmt --all -- --check
    if ($global:LASTEXITCODE -ne 0) {
        exit $global:LASTEXITCODE
    }

    Write-Host 'Running cargo clippy'
    $global:LASTEXITCODE = 0
    & cargo clippy --locked --all-targets --all-features -- -D warnings
    if ($global:LASTEXITCODE -ne 0) {
        exit $global:LASTEXITCODE
    }

    Write-Host 'Running cargo nextest'
    $global:LASTEXITCODE = 0
    & cargo nextest run --locked --all-targets --all-features
    if ($global:LASTEXITCODE -ne 0) {
        exit $global:LASTEXITCODE
    }
} finally {
    Pop-Location
}
