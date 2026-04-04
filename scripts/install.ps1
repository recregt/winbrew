$ErrorActionPreference = 'Stop'

[CmdletBinding()]
param(
    [string]$Repository = 'recregt/winbrew',
    [string]$InstallRoot = 'C:\winbrew',
    [switch]$Force
)

function Write-Info {
    param([string]$Message)
    Write-Host $Message
}

function Get-LatestBinaryReleaseAsset {
    param(
        [string]$Repo
    )

    $headers = @{ 'User-Agent' = 'winbrew-installer' }
    $releases = Invoke-RestMethod -Headers $headers -Uri "https://api.github.com/repos/$Repo/releases?per_page=100"
    $release = $releases | Where-Object { $_.tag_name -match '^v' -and -not $_.draft -and -not $_.prerelease } | Select-Object -First 1

    if (-not $release) {
        throw "Could not find a published binary release for $Repo."
    }

    $zipAsset = $release.assets | Where-Object { $_.name -match '^winbrew-.*-windows-x86_64\.zip$' } | Select-Object -First 1

    if (-not $zipAsset) {
        throw "Could not find a Windows release asset in the latest binary release for $Repo."
    }

    $checksumAsset = $release.assets | Where-Object { $_.name -eq ($zipAsset.name + '.sha256') } | Select-Object -First 1

    [pscustomobject]@{
        TagName = $release.tag_name
        ZipAsset = $zipAsset
        ChecksumAsset = $checksumAsset
    }
}

function Ensure-Directory {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        New-Item -ItemType Directory -Path $Path | Out-Null
    }
}

function Add-UserPathEntry {
    param([string]$Path)

    $current = [Environment]::GetEnvironmentVariable('Path', 'User')
    $entries = @()

    if ($current) {
        $entries = $current -split ';' | Where-Object { $_ -and $_.Trim() }
    }

    if ($entries -notcontains $Path) {
        $updated = @($entries + $Path) -join ';'
        [Environment]::SetEnvironmentVariable('Path', $updated, 'User')
    }

    if ($env:Path -notlike "*$Path*") {
        $env:Path = "$env:Path;$Path"
    }
}

function Grant-CurrentUserAccess {
    param([string]$Path)

    $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
    & icacls $Path /grant "$identity:(OI)(CI)F" /T /C | Out-Null
}

function Expand-ZipToTemp {
    param(
        [string]$ZipFile,
        [string]$Destination
    )

    if (Test-Path -LiteralPath $Destination) {
        Remove-Item -LiteralPath $Destination -Recurse -Force
    }

    Expand-Archive -LiteralPath $ZipFile -DestinationPath $Destination -Force
}

Write-Info "Fetching latest binary release from GitHub..."
$release = Get-LatestBinaryReleaseAsset -Repo $Repository

$binDir = Join-Path $InstallRoot 'bin'
$packagesDir = Join-Path $InstallRoot 'packages'
$dataDir = Join-Path $InstallRoot 'data'
$dbDir = Join-Path $dataDir 'db'
$logsDir = Join-Path $dataDir 'logs'
$cacheDir = Join-Path $dataDir 'cache'

Write-Info "Preparing install layout under $InstallRoot..."
Ensure-Directory -Path $InstallRoot
Ensure-Directory -Path $binDir
Ensure-Directory -Path $packagesDir
Ensure-Directory -Path $dataDir
Ensure-Directory -Path $dbDir
Ensure-Directory -Path $logsDir
Ensure-Directory -Path $cacheDir

$tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("winbrew-install-" + [Guid]::NewGuid().ToString('N'))
Ensure-Directory -Path $tempRoot

$zipPath = Join-Path $tempRoot $release.ZipAsset.name
$extractPath = Join-Path $tempRoot 'extract'

Write-Info "Downloading $($release.ZipAsset.name)..."
Invoke-WebRequest -Uri $release.ZipAsset.browser_download_url -OutFile $zipPath

if ($release.ChecksumAsset) {
    Write-Info 'Verifying archive checksum...'
    $checksumPath = Join-Path $tempRoot $release.ChecksumAsset.name
    Invoke-WebRequest -Uri $release.ChecksumAsset.browser_download_url -OutFile $checksumPath
    $expectedHash = (Get-Content -LiteralPath $checksumPath -Raw).Trim()
    $actualHash = (Get-FileHash -Algorithm SHA256 -LiteralPath $zipPath).Hash

    if ($expectedHash -ne $actualHash) {
        throw "Checksum mismatch for $($release.ZipAsset.name). Expected $expectedHash, got $actualHash."
    }
}

Write-Info 'Extracting archive...'
Expand-ZipToTemp -ZipFile $zipPath -Destination $extractPath

$winbrewExe = Get-ChildItem -Path $extractPath -Recurse -Filter 'winbrew.exe' | Select-Object -First 1

if (-not $winbrewExe) {
    throw 'winbrew.exe was not found in the downloaded archive.'
}

$targetExe = Join-Path $binDir 'winbrew.exe'
if ((Test-Path -LiteralPath $targetExe) -and -not $Force) {
    throw "$targetExe already exists. Re-run with -Force to overwrite it."
}

Write-Info "Installing winbrew.exe to $targetExe..."
Copy-Item -LiteralPath $winbrewExe.FullName -Destination $targetExe -Force

Write-Info 'Applying permissions...'
Grant-CurrentUserAccess -Path $InstallRoot

Write-Info 'Adding bin directory to user PATH...'
Add-UserPathEntry -Path $binDir

Remove-Item -LiteralPath $tempRoot -Recurse -Force

Write-Info ''
Write-Info "Winbrew $($release.TagName) installed successfully."
Write-Info "Binary: $targetExe"
Write-Info "Root:    $InstallRoot"
Write-Info 'Open a new terminal to use winbrew from PATH.'