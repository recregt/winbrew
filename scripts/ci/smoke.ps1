[CmdletBinding()]
param(
    [string]$ROOT = 'target\winbrew-ci',
    [string]$BinaryPath = 'target\x86_64-pc-windows-msvc\release\winbrew.exe'
)

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path
$SmokeScript = (Resolve-Path (Join-Path $PSScriptRoot '..\tasks\smoke.ps1')).Path

Push-Location $RepoRoot
try {
    $env:CARGO_BUILD_TARGET = 'x86_64-pc-windows-msvc'

    Write-Host 'Building release binary'
    $global:LASTEXITCODE = 0
    & cargo build --locked --release
    if ($global:LASTEXITCODE -ne 0) {
        exit $global:LASTEXITCODE
    }

    Write-Host 'Running CLI smoke test'
    $global:LASTEXITCODE = 0
    & $SmokeScript -BinaryPath $BinaryPath -ROOT $ROOT
    if ($global:LASTEXITCODE -ne 0) {
        exit $global:LASTEXITCODE
    }
} finally {
    Pop-Location
}
