[CmdletBinding()]
param(
    [switch]$SkipNextest
)

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

Push-Location $RepoRoot
try {
    $env:CARGO_BUILD_TARGET = 'x86_64-pc-windows-msvc'

    Write-Host 'Ensuring x86_64-pc-windows-msvc target is installed'
    $global:LASTEXITCODE = 0
    & rustup target add x86_64-pc-windows-msvc
    if ($global:LASTEXITCODE -ne 0) {
        exit $global:LASTEXITCODE
    }

    Write-Host 'Running cargo fmt'
    $global:LASTEXITCODE = 0
    & cargo fmt --all -- --check
    if ($global:LASTEXITCODE -ne 0) {
        exit $global:LASTEXITCODE
    }

    Write-Host 'Running cargo clippy'
    $global:LASTEXITCODE = 0
    & cargo clippy --locked --all-targets --all-features -p winbrew-cli -p winbrew-bin -- -D warnings
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

    if (-not $SkipNextest) {
        Write-Host 'Running cargo nextest'
        $global:LASTEXITCODE = 0
        & cargo nextest run --locked --all-targets --all-features -p winbrew-cli
        if ($global:LASTEXITCODE -ne 0) {
            exit $global:LASTEXITCODE
        }
    }
} finally {
    Pop-Location
}
