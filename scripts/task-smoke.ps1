[CmdletBinding()]
param(
    [string]$ROOT = 'target\winbrew-ci',

    [string]$BinaryPath = 'target\x86_64-pc-windows-msvc\debug\winbrew.exe'
)

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path

function Resolve-BinaryPath {
    param([string]$Path)

    if ([System.IO.Path]::IsPathRooted($Path)) {
        return $Path
    }

    return (Join-Path $RepoRoot $Path)
}

function Invoke-WinbrewCommand {
    param(
        [Parameter(Mandatory = $true)]
        [string[]]$Arguments
    )

    $global:LASTEXITCODE = 0
    & $script:ResolvedBinaryPath @Arguments

    if ($global:LASTEXITCODE -ne 0) {
        exit $global:LASTEXITCODE
    }
}

$ResolvedBinaryPath = Resolve-BinaryPath -Path $BinaryPath
$ResolvedRootPath = Resolve-BinaryPath -Path $ROOT
$PreviousWinbrewRoot = $env:WINBREW_PATHS_ROOT
$HadPreviousWinbrewRoot = Test-Path env:WINBREW_PATHS_ROOT

$env:WINBREW_PATHS_ROOT = $ResolvedRootPath

Push-Location $RepoRoot
try {
    $steps = @(
        [pscustomobject]@{ Label = '--version'; Arguments = @('--version') }
        [pscustomobject]@{ Label = '--help'; Arguments = @('--help') }
        [pscustomobject]@{ Label = 'version'; Arguments = @('version') }
        [pscustomobject]@{ Label = 'list'; Arguments = @('list') }
        [pscustomobject]@{ Label = 'list winbrew'; Arguments = @('list', 'winbrew') }
        [pscustomobject]@{ Label = 'search winbrew'; Arguments = @('search', 'winbrew') }
        [pscustomobject]@{ Label = 'info'; Arguments = @('info') }
        [pscustomobject]@{ Label = 'doctor'; Arguments = @('doctor') }
        [pscustomobject]@{ Label = 'update --help'; Arguments = @('update', '--help') }
        [pscustomobject]@{ Label = 'remove --help'; Arguments = @('remove', '--help') }
        [pscustomobject]@{ Label = 'config --help'; Arguments = @('config', '--help') }
        [pscustomobject]@{ Label = 'config list'; Arguments = @('config', 'list') }
        [pscustomobject]@{ Label = 'config get core.log_level'; Arguments = @('config', 'get', 'core.log_level') }
        [pscustomobject]@{ Label = 'config set core.default_yes true'; Arguments = @('config', 'set', 'core.default_yes', 'true') }
        [pscustomobject]@{ Label = 'config set core.color false'; Arguments = @('config', 'set', 'core.color', 'false') }
        [pscustomobject]@{ Label = 'config set core.log_level debug'; Arguments = @('config', 'set', 'core.log_level', 'debug') }
        [pscustomobject]@{ Label = 'config set core.file_log_level warn'; Arguments = @('config', 'set', 'core.file_log_level', 'warn') }
    )

    foreach ($step in $steps) {
        Write-Host "Running: $($step.Label)"
        Invoke-WinbrewCommand -Arguments $step.Arguments
    }
} finally {
    Pop-Location

    if ($HadPreviousWinbrewRoot) {
        $env:WINBREW_PATHS_ROOT = $PreviousWinbrewRoot
    } else {
        Remove-Item Env:WINBREW_PATHS_ROOT -ErrorAction SilentlyContinue
    }
}