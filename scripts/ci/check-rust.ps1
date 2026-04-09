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
    & cargo clippy --locked --all-targets --all-features -p winbrew -- -D warnings
    if ($global:LASTEXITCODE -ne 0) {
        exit $global:LASTEXITCODE
    }

    Write-Host 'Running cargo doc'
    $previousRUSTDOCFLAGS = $env:RUSTDOCFLAGS
    try {
        $env:RUSTDOCFLAGS = '-D warnings'
        $global:LASTEXITCODE = 0
        & cargo doc --locked --workspace --no-deps
        if ($global:LASTEXITCODE -ne 0) {
            exit $global:LASTEXITCODE
        }
    } finally {
        $env:RUSTDOCFLAGS = $previousRUSTDOCFLAGS
    }

    Write-Host 'Running cargo nextest'
    $global:LASTEXITCODE = 0
    & cargo nextest run --locked --all-targets --all-features -p winbrew
    if ($global:LASTEXITCODE -ne 0) {
        exit $global:LASTEXITCODE
    }
} finally {
    Pop-Location
}
