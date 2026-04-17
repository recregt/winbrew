[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('crawler', 'publisher')]
    [string]$Module
)

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..\..')).Path
$ModulePath = Join-Path $RepoRoot "infra\$Module"
$LintConfigPath = Join-Path $RepoRoot 'infra\golangci.yml'

if (-not (Get-Command golangci-lint -ErrorAction SilentlyContinue)) {
    throw 'golangci-lint was not found in PATH'
}

Push-Location $ModulePath
try {
    Write-Host "Running golangci-lint for $Module"
    $global:LASTEXITCODE = 0
    & golangci-lint run "--config=$LintConfigPath" ./...
    if ($global:LASTEXITCODE -ne 0) {
        exit $global:LASTEXITCODE
    }

    Write-Host "Running go test for $Module"
    $global:LASTEXITCODE = 0
    & go test ./...
    if ($global:LASTEXITCODE -ne 0) {
        exit $global:LASTEXITCODE
    }
} finally {
    Pop-Location
}
