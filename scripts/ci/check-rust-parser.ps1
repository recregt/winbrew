[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$ParserManifest = Join-Path $RepoRoot 'infra\parser\Cargo.toml'

Push-Location $RepoRoot
try {
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
