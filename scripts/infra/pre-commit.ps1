[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..\..')).Path

Push-Location $RepoRoot
try {
    $files = git -C $RepoRoot ls-files 'infra/crawler/*.go' 'infra/crawler/**/*.go' 'infra/publisher/*.go' 'infra/publisher/**/*.go'
    if ($files) {
        foreach ($file in $files) {
            $global:LASTEXITCODE = 0
            $formatted = & gofmt -l (Join-Path $RepoRoot $file)
            if ($global:LASTEXITCODE -ne 0) {
                exit $global:LASTEXITCODE
            }
            if ($formatted) {
                Write-Host "Needs gofmt: $file"
                exit 1
            }
        }
    }

    foreach ($module in @('infra\crawler', 'infra\publisher')) {
        Push-Location $module
        try {
            $global:LASTEXITCODE = 0
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

    $global:LASTEXITCODE = 0
    cargo test --workspace
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
} finally {
    Pop-Location
}