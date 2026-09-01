<#
.SYNOPSIS
Installs a built Palworld Guider UE4SS package into the game Mods directory
after verifying a fresh save backup.

.DESCRIPTION
Refuses to run while Palworld is running, requires a verified non-empty
backup produced by Backup-G5Save.ps1 (backup-manifest.json) before any
write to the game directory, and requires the package SHA256 manifest
produced by Build-G5Ue4ss.ps1. Installs only into the game Mods directory
under the exact PalworldGuider subfolder, using -LiteralPath, and updates
mods.txt to enable only the PalworldGuider line while preserving every other
mod's line and folder. On failure before
completion, the added package and mods.txt changes are rolled back.

.PARAMETER PackageDirectory
The built PalworldGuider package directory (from Build-G5Ue4ss.ps1).

.PARAMETER ModsDirectory
The game's UE4SS Mods directory (for example
D:\Steam\steamapps\common\Palworld\Pal\Binaries\Win64\Mods).

.PARAMETER BackupDirectory
A verified backup directory produced by Backup-G5Save.ps1.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory = $true)]
    [string]$PackageDirectory,
    [Parameter(Mandatory = $true)]
    [string]$ModsDirectory,
    [Parameter(Mandatory = $true)]
    [string]$BackupDirectory
)

$ErrorActionPreference = 'Stop'

$RunningGame = Get-Process -Name 'Palworld*' -ErrorAction SilentlyContinue
if ($RunningGame) {
    throw "refusing to install while Palworld is running"
}

$BackupManifestPath = Join-Path $BackupDirectory 'backup-manifest.json'
if (-not (Test-Path -LiteralPath $BackupManifestPath)) {
    throw "refusing to install: no verified backup at $BackupDirectory (missing backup-manifest.json); run Backup-G5Save.ps1 first"
}
$BackupRecord = Get-Content -LiteralPath $BackupManifestPath -Raw | ConvertFrom-Json
if (-not $BackupRecord.completed_at -or -not $BackupRecord.file_count -or $BackupRecord.file_count -lt 1) {
    throw "refusing to install: no verified backup at $BackupDirectory (manifest is incomplete)"
}
$BackupFiles = @(Get-ChildItem -LiteralPath $BackupDirectory -Recurse -File -Force)
if ($BackupFiles.Count -lt 1) {
    throw "refusing to install: no verified backup at $BackupDirectory (directory is empty)"
}

$PackageManifestPath = Join-Path (Split-Path -Parent $PackageDirectory) 'PalworldGuider.sha256'
if (-not (Test-Path -LiteralPath $PackageManifestPath)) {
    throw "refusing to install: package hash manifest not found at $PackageManifestPath; run Build-G5Ue4ss.ps1 first"
}
$ExpectedHashes = @{}
foreach ($manifestLine in @(Get-Content -LiteralPath $PackageManifestPath)) {
    $parts = $manifestLine -split '\s+', 2
    if ($parts.Count -eq 2) {
        $ExpectedHashes[$parts[1]] = $parts[0].ToLowerInvariant()
    }
}
if ($ExpectedHashes.Count -eq 0) {
    throw "refusing to install: package manifest contains no verifiable entries: $PackageManifestPath"
}
foreach ($relativePath in $ExpectedHashes.Keys) {
    $stagedFile = Join-Path $PackageDirectory $relativePath
    if (-not (Test-Path -LiteralPath $stagedFile)) {
        throw "refusing to install: staged package file missing: $stagedFile"
    }
    $actualHash = (Get-FileHash -LiteralPath $stagedFile -Algorithm SHA256).Hash.ToLowerInvariant()
    if ($actualHash -ne $ExpectedHashes[$relativePath]) {
        throw "refusing to install: package hash mismatch for $stagedFile"
    }
}

$TargetDirectory = Join-Path $ModsDirectory 'PalworldGuider'
if (Test-Path -LiteralPath $TargetDirectory) {
    throw "refusing to install: Mods\PalworldGuider already exists at $TargetDirectory; run Uninstall-G5Ue4ss.ps1 first"
}
$ModsFile = Join-Path $ModsDirectory 'mods.txt'
$OriginalModsText = if (Test-Path -LiteralPath $ModsFile) { Get-Content -LiteralPath $ModsFile -Raw } else { $null }

try {
    New-Item -ItemType Directory -Path $TargetDirectory -Force | Out-Null
    foreach ($packageChild in @(Get-ChildItem -LiteralPath $PackageDirectory -Force)) {
        Copy-Item -LiteralPath $packageChild.FullName -Destination $TargetDirectory -Recurse -Force
    }

    $ExistingLines = if (Test-Path -LiteralPath $ModsFile) { @(Get-Content -LiteralPath $ModsFile) } else { @() }
    $KeptLines = @($ExistingLines | Where-Object { $_ -notmatch '^\s*PalworldGuider\s*:' })
    $UpdatedLines = @($KeptLines) + @('PalworldGuider : 1')
    Set-Content -LiteralPath $ModsFile -Value $UpdatedLines -Encoding Ascii

    Write-Host "Installed PalworldGuider into Mods\PalworldGuider at $TargetDirectory and enabled it in $ModsFile"
}
catch {
    Write-Warning "Install failed; rolling back PalworldGuider additions"
    if (Test-Path -LiteralPath (Join-Path $ModsDirectory 'PalworldGuider')) {
        Remove-Item -LiteralPath (Join-Path $ModsDirectory 'PalworldGuider') -Recurse -Force
    }
    if ($null -eq $OriginalModsText) {
        if (Test-Path -LiteralPath $ModsFile) {
            Remove-Item -LiteralPath $ModsFile -Force
        }
    }
    elseif (Test-Path -LiteralPath $ModsFile) {
        Set-Content -LiteralPath $ModsFile -Value $OriginalModsText -Encoding Ascii
    }
    throw
}
