$ErrorActionPreference = 'Stop'

[CmdletBinding()]
param(
    [string]$Repository = 'recregt/winbrew',
    [string]$InstallRoot = 'C:\winbrew',
    [switch]$Force
)

Set-StrictMode -Version Latest

$InstallRoot = [System.IO.Path]::GetFullPath($InstallRoot)

function Write-Info {
    param([string]$Message)
    Write-Host $Message
}

function Assert-CommandAvailable {
    param([string]$Name)

    if (-not (Get-Command -Name $Name -ErrorAction SilentlyContinue)) {
        throw "Required command '$Name' was not found on PATH."
    }
}

function Test-Administrator {
    try {
        $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent()
        $principal = [System.Security.Principal.WindowsPrincipal]::new($identity)
        return $principal.IsInRole([System.Security.Principal.WindowsBuiltInRole]::Administrator)
    }
    catch {
        return $false
    }
}

function Resolve-RepositoryUri {
    param([string]$Repo)

    $normalizedRepo = $Repo.Trim()

    switch -Regex ($normalizedRepo) {
        '^recregt/winbrew(?:\.git)?$' {
            return 'https://github.com/recregt/winbrew.git'
        }
        '^https://github\.com/recregt/winbrew(?:\.git)?$' {
            return 'https://github.com/recregt/winbrew.git'
        }
        '^git@github\.com:recregt/winbrew(?:\.git)?$' {
            return 'https://github.com/recregt/winbrew.git'
        }
        '^ssh://git@github\.com/recregt/winbrew(?:\.git)?$' {
            return 'https://github.com/recregt/winbrew.git'
        }
        default {
            throw "Repository '$Repo' is not allowed. Use recregt/winbrew or the canonical Winbrew GitHub URL."
        }
    }
}

function Ensure-Directory {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        New-Item -ItemType Directory -Path $Path -Force | Out-Null
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

    if ([string]::IsNullOrWhiteSpace($env:Path)) {
        $env:Path = $Path
    }
    else {
        $envEntries = $env:Path -split ';' | Where-Object { $_ -and $_.Trim() }
        if ($envEntries -notcontains $Path) {
            $env:Path = @($envEntries + $Path) -join ';'
        }
    }
}

function Remove-UserPathEntry {
    param([string]$Path)

    $current = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($current) {
        $entries = $current -split ';' | Where-Object { $_ -and $_.Trim() -and $_ -ne $Path }
        [Environment]::SetEnvironmentVariable('Path', @($entries) -join ';', 'User')
    }

    if ($env:Path) {
        $envEntries = $env:Path -split ';' | Where-Object { $_ -and $_.Trim() -and $_ -ne $Path }
        $env:Path = @($envEntries) -join ';'
    }
}

function Remove-InstallRootPathEntries {
    param([string]$InstallRoot)

    Remove-UserPathEntry -Path (Join-Path $InstallRoot 'bin')
    Remove-UserPathEntry -Path $InstallRoot
}

function Grant-CurrentUserAccess {
    param([string]$Path)

    $identity = [System.Security.Principal.WindowsIdentity]::GetCurrent().Name
    & icacls $Path /grant "$identity:(OI)(CI)F" /T /C | Out-Null
}

function Assert-SufficientDiskSpace {
    param([string]$Path)

    $minimumFreeBytes = 3GB
    $pathRoot = [System.IO.Path]::GetPathRoot($Path)

    if (-not $pathRoot) {
        throw "Unable to determine the drive root for '$Path'."
    }

    $driveName = $pathRoot.TrimEnd('\').TrimEnd(':')
    $drive = Get-PSDrive -Name $driveName -ErrorAction Stop
    $freeBytes = [int64]$drive.Free

    if ($freeBytes -lt $minimumFreeBytes) {
        $requiredMb = [math]::Round($minimumFreeBytes / 1MB, 0)
        $availableMb = [math]::Round($freeBytes / 1MB, 0)
        throw "Not enough free disk space on $driveName:. Required at least $requiredMb MB, available $availableMb MB."
    }
}

function Assert-RepositoryReachable {
    param([string]$RepositoryUri)

    Write-Info "Checking repository access..."
    & git ls-remote --heads $RepositoryUri 1>$null 2>$null

    if ($LASTEXITCODE -ne 0) {
        throw "Unable to reach repository '$RepositoryUri'."
    }
}

function New-TempRoot {
    $tempRoot = Join-Path ([System.IO.Path]::GetTempPath()) ("winbrew-install-" + [Guid]::NewGuid().ToString('N'))
    Ensure-Directory -Path $tempRoot
    return $tempRoot
}

function Invoke-GitClone {
    param(
        [string]$RepositoryUri,
        [string]$Destination
    )

    Write-Info "Cloning $RepositoryUri..."
    & git clone --depth 1 --single-branch $RepositoryUri $Destination
}

function Invoke-CargoBuild {
    param([string]$SourceRoot)

    Write-Info 'Building release binary...'
    Push-Location $SourceRoot
    try {
        & cargo build --release --locked --bin winbrew -p winbrew-bin
    }
    finally {
        Pop-Location
    }
}

function Invoke-VersionSmokeTest {
    param(
        [string]$BinaryPath,
        [string]$Label
    )

    $versionOutput = & $BinaryPath --version 2>$null
    if ($LASTEXITCODE -ne 0) {
        throw "$Label failed version verification at '$BinaryPath'."
    }

    if (-not $versionOutput) {
        throw "$Label did not produce version output at '$BinaryPath'."
    }

    $versionText = ($versionOutput | Select-Object -First 1).ToString()
    Write-Info "$Label version: $versionText"
}

function Remove-InstallRoot {
    param([string]$Path)

    if (Test-Path -LiteralPath $Path) {
        Remove-InstallRootPathEntries -InstallRoot $Path
        Remove-Item -LiteralPath $Path -Recurse -Force
    }
}

Assert-CommandAvailable -Name 'git'
Assert-CommandAvailable -Name 'cargo'

if (-not (Test-Administrator)) {
    Write-Warning 'This script is usually run from an elevated PowerShell session so permission setup is predictable.'
}

$repositoryUri = Resolve-RepositoryUri -Repo $Repository

if ((Test-Path -LiteralPath $InstallRoot) -and -not $Force) {
    throw "$InstallRoot already exists. Re-run with -Force to replace it cleanly."
}

Assert-SufficientDiskSpace -Path $InstallRoot
Assert-SufficientDiskSpace -Path ([System.IO.Path]::GetTempPath())
Assert-RepositoryReachable -RepositoryUri $repositoryUri

$tempRoot = New-TempRoot
$sourceRoot = Join-Path $tempRoot 'source'
$builtExe = Join-Path $sourceRoot 'target\release\winbrew.exe'
$targetExe = Join-Path $InstallRoot 'winbrew.exe'
$installRootPrepared = $false

try {
    Invoke-GitClone -RepositoryUri $repositoryUri -Destination $sourceRoot
    Invoke-CargoBuild -SourceRoot $sourceRoot

    if (-not (Test-Path -LiteralPath $builtExe)) {
        throw "Built executable was not found at $builtExe."
    }

    Invoke-VersionSmokeTest -BinaryPath $builtExe -Label 'Built binary'

    Write-Info "Preparing install root at $InstallRoot..."
    if (Test-Path -LiteralPath $InstallRoot) {
        Remove-InstallRoot -Path $InstallRoot
    }

    Ensure-Directory -Path $InstallRoot
    $installRootPrepared = $true

    Write-Info "Installing winbrew.exe to $targetExe..."
    Copy-Item -LiteralPath $builtExe -Destination $targetExe -Force

    Invoke-VersionSmokeTest -BinaryPath $targetExe -Label 'Installed binary'

    Write-Info 'Applying permissions...'
    Grant-CurrentUserAccess -Path $InstallRoot

    Write-Info 'Updating user PATH...'
    Remove-InstallRootPathEntries -InstallRoot $InstallRoot
    Add-UserPathEntry -Path $InstallRoot

    Write-Info ''
    Write-Info 'Winbrew built and installed successfully.'
    Write-Info "Binary: $targetExe"
    Write-Info "Root:    $InstallRoot"
    Write-Info 'The source checkout, build output, and temp workspace were cleaned up automatically.'
}
catch {
    if ($installRootPrepared) {
        try {
            Remove-InstallRoot -Path $InstallRoot
        }
        catch {
        }
    }

    throw
}
finally {
    if (Test-Path -LiteralPath $tempRoot) {
        Remove-Item -LiteralPath $tempRoot -Recurse -Force
    }
}
