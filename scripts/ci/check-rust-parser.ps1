[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$ParserManifest = Join-Path $RepoRoot 'infra\parser\Cargo.toml'
$HostTarget = (& rustc -vV | Select-String '^host: ' | ForEach-Object { $_.ToString().Split(': ', 2)[1].Trim() })

if (-not $HostTarget) {
    throw 'failed to detect the Rust host target'
}

Push-Location $RepoRoot
try {
    $env:CARGO_BUILD_TARGET = $HostTarget

    Write-Host "Using Cargo target $HostTarget"

    Write-Host 'Running parser cargo fmt'
    $global:LASTEXITCODE = 0
    & cargo fmt --manifest-path $ParserManifest -- --check
    if ($global:LASTEXITCODE -ne 0) {
        exit $global:LASTEXITCODE
    }

    Write-Host 'Running parser cargo clippy'
    $global:LASTEXITCODE = 0
    & cargo clippy --manifest-path $ParserManifest --locked --all-targets --all-features -- -D warnings
    if ($global:LASTEXITCODE -ne 0) {
        exit $global:LASTEXITCODE
    }

    Write-Host 'Running parser cargo test'
    $global:LASTEXITCODE = 0
    & cargo test --manifest-path $ParserManifest --locked --all-targets --all-features
    if ($global:LASTEXITCODE -ne 0) {
        exit $global:LASTEXITCODE
    }
} finally {
    Pop-Location
}
