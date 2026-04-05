[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$MsgFile
)

$ErrorActionPreference = 'Stop'

$message = Get-Content -TotalCount 1 -LiteralPath $MsgFile
$pattern = '^(feat|fix|docs|style|refactor|perf|test|build|ci|chore|revert)(\([a-z0-9._/-]+\))?!?: .+'

if ($message -notmatch $pattern) {
    Write-Host "Invalid commit message: $message"
    Write-Host 'Expected Conventional Commits format, e.g.:'
    Write-Host '  feat(cli): rename binary to winbrew'
    Write-Host '  fix(cleaner)!: remove legacy behavior'
    exit 1
}