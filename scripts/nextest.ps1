[CmdletBinding()]
param(
    [ValidateSet('install', 'run')]
    [string]$Action = 'run',

    [Parameter(ValueFromRemainingArguments = $true)]
    [string[]]$NextestArgs = @()
)

$ErrorActionPreference = 'Stop'

$RepoRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$InstallRoot = Join-Path $env:LOCALAPPDATA 'winbrew\nextest'
$VersionFile = Join-Path $InstallRoot 'version.txt'
$BinaryPath = Join-Path $InstallRoot 'cargo-nextest.exe'
$TargetTriple = 'x86_64-pc-windows-msvc'
$NextestVersion = '0.9.132'
$ReleaseTag = "cargo-nextest-$NextestVersion"
$ReleaseBaseUrl = "https://github.com/nextest-rs/nextest/releases/download/$ReleaseTag"

function Ensure-Directory {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        New-Item -ItemType Directory -Path $Path | Out-Null
    }
}

function Install-Nextest {
    Ensure-Directory -Path $InstallRoot

    if ((Test-Path -LiteralPath $BinaryPath) -and (Test-Path -LiteralPath $VersionFile)) {
        $installedVersion = (Get-Content -LiteralPath $VersionFile -TotalCount 1).Trim()
        if ($installedVersion -eq $NextestVersion) {
            return $BinaryPath
        }
    }

    $assetName = "cargo-nextest-$NextestVersion-${TargetTriple}.zip"
    $assetUrl = "$ReleaseBaseUrl/$assetName"

    $downloadPath = Join-Path $InstallRoot "$assetName"
    $extractPath = Join-Path $InstallRoot "$NextestVersion-extracted"

    if (Test-Path -LiteralPath $extractPath) {
        Remove-Item -LiteralPath $extractPath -Recurse -Force
    }

    try {
        Invoke-WebRequest -Uri $assetUrl -OutFile $downloadPath
        Expand-Archive -LiteralPath $downloadPath -DestinationPath $extractPath -Force

        $extractedBinary = Get-ChildItem -Path $extractPath -Filter 'cargo-nextest.exe' -Recurse | Select-Object -First 1
        if (-not $extractedBinary) {
            throw 'Downloaded nextest archive did not contain cargo-nextest.exe.'
        }

        Copy-Item -LiteralPath $extractedBinary.FullName -Destination $BinaryPath -Force
        Set-Content -LiteralPath $VersionFile -Value $NextestVersion -NoNewline
    } finally {
        if (Test-Path -LiteralPath $downloadPath) {
            Remove-Item -LiteralPath $downloadPath -Force
        }

        if (Test-Path -LiteralPath $extractPath) {
            Remove-Item -LiteralPath $extractPath -Recurse -Force
        }
    }

    return $BinaryPath
}

$nextestBinary = Install-Nextest

if ($Action -eq 'install') {
    Write-Host "cargo-nextest installed at $nextestBinary"
    exit 0
}

Push-Location $RepoRoot
try {
    & $nextestBinary nextest run @NextestArgs
    if ($LASTEXITCODE -ne 0) {
        exit $LASTEXITCODE
    }
} finally {
    Pop-Location
}