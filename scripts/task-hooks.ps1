[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [ValidateSet('commit-msg', 'pre-commit', 'pre-push')]
    [string]$Hook,

    [string]$MsgFile
)

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

function Invoke-RepoCommand {
    param(
        [scriptblock]$Command
    )

    Push-Location $RepoRoot
    try {
        $global:LASTEXITCODE = 0
        & $Command
        if ($global:LASTEXITCODE -ne 0) {
            exit $global:LASTEXITCODE
        }
    } finally {
        Pop-Location
    }
}

function Test-ConventionalCommit {
    param(
        [Parameter(Mandatory = $true)]
        [string]$Path
    )

    $message = Get-Content -TotalCount 1 -LiteralPath $Path
    $pattern = '^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([a-z0-9._/-]+\))?!?: .+'

    if ($message -notmatch $pattern) {
        Write-Host "Invalid commit message: $message"
        Write-Host 'Expected Conventional Commits format, e.g.:'
        Write-Host '  feat(cli): add brew alias'
        Write-Host '  fix(cleaner)!: remove legacy behavior'
        exit 1
    }
}

function Test-InfraGofmt {
    $files = git -C $RepoRoot ls-files 'infra/*.go' 'infra/**/*.go'
    if (-not $files) {
        return
    }

    foreach ($file in $files) {
        $global:LASTEXITCODE = 0
        $formatted = & gofmt -l $file
        if ($global:LASTEXITCODE -ne 0) {
            exit $global:LASTEXITCODE
        }
        if ($formatted) {
            Write-Host "Needs gofmt: $file"
            exit 1
        }
    }
}

switch ($Hook) {
    'commit-msg' {
        if (-not $MsgFile) {
            throw 'MsgFile is required for the commit-msg hook.'
        }

        Test-ConventionalCommit -Path $MsgFile
    }

    'pre-commit' {
        Invoke-RepoCommand { cargo fmt --all -- --check }
        Invoke-RepoCommand { cargo check --locked --all-targets --all-features }
        Invoke-RepoCommand { Test-InfraGofmt }
    }

    'pre-push' {
        Invoke-RepoCommand { cargo clippy --all-targets --all-features -- -D warnings }
        Invoke-RepoCommand { cargo test --locked --all-targets --all-features }
        Invoke-RepoCommand {
            if (Test-Path infra/go.mod) {
                Push-Location infra
                try {
                    $global:LASTEXITCODE = 0
                    go vet ./...
                    if ($global:LASTEXITCODE -ne 0) {
                        exit $global:LASTEXITCODE
                    }
                } finally {
                    Pop-Location
                }
            }
        }
        Invoke-RepoCommand {
            if (Test-Path infra/go.mod) {
                Push-Location infra
                try {
                    $global:LASTEXITCODE = 0
                    go test ./...
                    if ($global:LASTEXITCODE -ne 0) {
                        exit $global:LASTEXITCODE
                    }
                } finally {
                    Pop-Location
                }
            }
        }
    }
}
